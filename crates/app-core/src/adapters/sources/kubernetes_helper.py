# Scoped, read-only kubectl adapter. Config is validated before any API access;
# credentials remain in a sealed memfd, never stdout or an on-disk snapshot.
import os, sys, json, subprocess, selectors, signal, time, hashlib, re, fcntl
p = json.loads(sys.argv[1])
children = []
config_fd = None
CAP = 256 * 1024

def send(value, data=b''):
    value['length'] = len(data)
    sys.stdout.buffer.write(json.dumps(value, separators=(',', ':')).encode() + b'\n')
    sys.stdout.buffer.write(data)
    sys.stdout.buffer.flush()

def close(child):
    if child.poll() is None:
        child.terminate()
        try: child.wait(timeout=.3)
        except subprocess.TimeoutExpired: child.kill(); child.wait()
    for pipe in (child.stdout, child.stderr):
        if pipe: pipe.close()
    if child in children: children.remove(child)

def cleanup(*_):
    global config_fd
    for child in children[:]: close(child)
    if config_fd is not None: os.close(config_fd); config_fd=None
    if _: sys.exit(0)
signal.signal(signal.SIGTERM, cleanup)
signal.signal(signal.SIGHUP, cleanup)

def launch(args, credentials=False):
    child = subprocess.Popen(args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        pass_fds=(config_fd,) if credentials else (), env={**os.environ, 'LC_ALL':'C', 'KUBECTL_KUBERC':'false'})
    children.append(child)
    return child

def bounded(args, credentials=False):
    child=launch(args, credentials)
    output=bytearray(); diagnostic=bytearray(); deadline=time.monotonic()+4
    sel=selectors.DefaultSelector(); sel.register(child.stdout, selectors.EVENT_READ, output); sel.register(child.stderr, selectors.EVENT_READ, diagnostic)
    try:
        while sel.get_map():
            if time.monotonic()>deadline: raise ValueError('kubernetes_probe_timeout')
            for key,_ in sel.select(.1):
                block=os.read(key.fileobj.fileno(),8192)
                if not block: sel.unregister(key.fileobj); continue
                key.data.extend(block)
                if len(output)>CAP or len(diagnostic)>4096: raise ValueError('kubernetes_response_cap')
        if child.wait(timeout=.2)!=0: raise ValueError('kubernetes_permission_or_capability_failure')
        return json.loads(output)
    finally: sel.close();close(child)

try:
    for name in ('context','namespace','pod','container'):
        value=p.get(name,'')
        if not isinstance(value,str) or not value or len(value)>253 or value.startswith('-') or any(ord(c)<33 for c in value): raise ValueError('invalid_selected_scope')
    path=p.get('kubeconfig','')
    if not isinstance(path,str) or not path.startswith('/') or len(path)>4096 or '\x00' in path: raise ValueError('explicit_absolute_kubeconfig_required')
    def config():
        value=bounded(['kubectl','config','view','--kubeconfig='+path,'--context='+p['context'],'--minify','--flatten','--raw','-o','json'])
        if len(value.get('contexts',[]))!=1 or value['contexts'][0]['name']!=p['context']: raise ValueError('selected_context_missing')
        for entry in value.get('users',[]):
            user=entry.get('user',{})
            if any(user.get(k) for k in ('exec','auth-provider','tokenFile','as','as-groups','as-user-extra')): raise ValueError('dynamic_credentials_unsupported')
        for entry in value.get('clusters',[]):
            cluster=entry.get('cluster',{})
            if cluster.get('insecure-skip-tls-verify') or not cluster.get('server','').startswith('https://'): raise ValueError('verified_tls_required')
        data=json.dumps(value,sort_keys=True,separators=(',',':')).encode()
        return data,hashlib.sha256(data).hexdigest()
    data,digest=config()
    if p.get('action')!='probe' and digest!=p.get('config_digest'): raise ValueError('kubeconfig_changed_reinspect_and_trust')
    if not hasattr(os,'memfd_create'): raise ValueError('linux_memfd_required')
    config_fd=os.memfd_create('tailpro-kubeconfig',os.MFD_CLOEXEC|os.MFD_ALLOW_SEALING)
    os.fchmod(config_fd,0o600);os.write(config_fd,data)
    fcntl.fcntl(config_fd,fcntl.F_ADD_SEALS,fcntl.F_SEAL_WRITE|fcntl.F_SEAL_GROW|fcntl.F_SEAL_SHRINK|fcntl.F_SEAL_SEAL)
    base=['kubectl','--kubeconfig=/proc/self/fd/'+str(config_fd),'--context='+p['context'],'--namespace='+p['namespace']]
    def identity():
        pod=bounded(base+['get','pod',p['pod'],'-o','json','--request-timeout=3s'],True)
        meta=pod.get('metadata',{})
        if meta.get('name')!=p['pod'] or meta.get('namespace')!=p['namespace']: raise ValueError('pod_identity_mismatch')
        statuses=pod.get('status',{}).get('containerStatuses',[])
        status=next((s for s in statuses if s.get('name')==p['container']),None)
        if status is None or not status.get('containerID'): raise ValueError('selected_container_not_running_or_unsupported')
        return {'pod_uid':meta.get('uid'), 'container_id':status['containerID'], 'restart_count':status.get('restartCount'), 'previous_available':bool(status.get('lastState',{}).get('terminated',{}).get('containerID')), 'previous_container_id':status.get('lastState',{}).get('terminated',{}).get('containerID')}
    selected=identity()
    if p.get('action')=='probe':
        send({'kind':'capability','config_digest':digest, **selected, 'coverage':'Selected normal container only; pods/get and pods/log/get required; latest file or one previous instance; identity checks cannot eliminate API name-reuse race; dynamic credentials unsupported'})
    else:
        if not p.get('trusted'): raise ValueError('explicit_context_trust_required')
        if selected['pod_uid']!=p.get('pod_uid') or selected['container_id']!=p.get('container_id') or selected['restart_count']!=p.get('restart_count'): raise ValueError('pod_or_container_instance_changed_reinspect')
        if p.get('previous') and (not selected['previous_available'] or selected['previous_container_id']!=p.get('previous_container_id')): raise ValueError('previous_instance_unavailable_or_changed')
        args=base+['logs',p['pod'],'--container='+p['container'],'--timestamps=true']
        if p.get('previous'): args+=['--previous=true','--tail=200']
        else: args+=['--follow=true']
        if p.get('since') and not p.get('previous'): args+=['--since-time='+p['since']]
        elif not p.get('previous'): args+=['--tail=200']
        child=launch(args,True)
        if identity()!=selected: raise ValueError('instance_changed_at_open_gap')
        send({'kind':'opened','coverage':'Merged API log stream; history/rotation and name-based API race remain uncertain; replay retained'})
        sel=selectors.DefaultSelector();sel.register(child.stdout,selectors.EVENT_READ,'data');sel.register(child.stderr,selectors.EVENT_READ,'diagnostic')
        next_check=time.monotonic()+2
        while sel.get_map():
            for key,_ in sel.select(.1):
                block=os.read(key.fileobj.fileno(),16384)
                if not block: sel.unregister(key.fileobj);continue
                if key.data=='diagnostic': raise ValueError('kubernetes_log_permission_or_transport_error')
                send({'kind':'data'},block)
            if time.monotonic()>=next_check:
                if config()[1]!=digest: raise ValueError('kubeconfig_changed_during_follow_gap')
                if identity()!=selected: raise ValueError('restart_or_recreate_gap_reinspect_instance')
                next_check=time.monotonic()+2
        sel.close()
        if child.wait(timeout=.3)!=0: raise ValueError('kubernetes_log_failure_gap')
        if identity()!=selected: raise ValueError('instance_changed_at_end_gap')
        send({'kind':'ended','previous':bool(p.get('previous'))})
except FileNotFoundError:
    send({'kind':'unsupported','reason':'kubectl_or_kubeconfig_unavailable'})
except (ValueError,OSError,subprocess.SubprocessError) as error:
    reason=str(error)
    # Only fixed internal reason codes leave the helper, never command stderr/config.
    if not re.fullmatch('[a-z_]+',reason): reason='kubernetes_protocol_or_capability_failure'
    send({'kind':'needs_action','reason':reason})
finally: cleanup()
