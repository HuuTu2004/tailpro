#!/usr/bin/env python3
"""Manually invoked synthetic CLI observations. No real daemon/cluster/SSH or assertions.
Run: python3 fixtures/universal-ingestion/simulate_sources.py
"""
import json, os, pathlib, subprocess, tempfile, time, select
ROOT=pathlib.Path(__file__).resolve().parents[2]
HELPERS=ROOT/'crates/app-core/src/adapters/sources'
STUB=r'''#!/usr/bin/env python3
import json,os,sys,time,pathlib
args=sys.argv[1:];name=pathlib.Path(sys.argv[0]).name
with open(os.environ['FIXTURE_CALLS'],'a') as f:f.write(json.dumps({'name':name,'args':args,'pid':os.getpid()})+'\n')
mode=os.environ.get('FIXTURE_MODE','normal')
if name=='journalctl':
    if any(a.startswith('--cursor=') for a in args):
        print(json.dumps({'__CURSOR':'different-cursor','MESSAGE':'cursor probe'}))
    elif mode=='oversize':print(json.dumps({'__CURSOR':'oversize','MESSAGE':'x'*150000}))
    else:
        for item in [
          {'__CURSOR':'entry-1','MESSAGE':'{"time":"2026-11-20T16:00:00Z","level":"info","message":"journal app"}','__REALTIME_TIMESTAMP':'1795194001123456','PRIORITY':'3'},
          {'__CURSOR':'entry-2','MESSAGE':[97,255,0,13,10],'PRIORITY':'6'},
          {'__CURSOR':'entry-3','MESSAGE':['first value','second value'],'PRIORITY':'4'}]:print(json.dumps(item),flush=True)
elif name=='docker':
    if mode=='denied':print('synthetic permission failure',file=sys.stderr);sys.exit(1)
    print(json.dumps([{'Id':'a'*64,'Config':{'Tty':mode=='tty'},'HostConfig':{'LogConfig':{'Type':'none' if mode=='none' else 'json-file'}}}]))
elif name=='kubectl':
    if 'config' in args:
        user={'token':'SYNTHETIC_CANARY_TOKEN'}
        if mode=='plugin':user={'exec':{'command':'synthetic-must-never-execute'}}
        print(json.dumps({'apiVersion':'v1','kind':'Config','current-context':'fixture','contexts':[{'name':'fixture','context':{'cluster':'fixture','namespace':'selected','user':'fixture'}}],'clusters':[{'name':'fixture','cluster':{'server':'https://cluster.invalid'}}],'users':[{'name':'fixture','user':user}]}))
    elif 'get' in args:
        if mode=='denied':print('synthetic forbidden',file=sys.stderr);sys.exit(1)
        state=pathlib.Path(os.environ['FIXTURE_STATE']);n=int(state.read_text())+1 if state.exists() else 1;state.write_text(str(n))
        restart=1 if mode=='restart' and n>=3 else 0
        print(json.dumps({'metadata':{'name':'pod','namespace':'selected','uid':'uid-fixture'},'status':{'containerStatuses':[{'name':'app','containerID':'containerd://fixture-'+str(restart),'restartCount':restart,'lastState':{'terminated':{'containerID':'containerd://previous'}}}]}}))
    elif 'logs' in args:
        # Validate the snapshotted memfd is the only credential input seen by this invocation.
        filename=next(a.split('=',1)[1] for a in args if a.startswith('--kubeconfig='))
        with open(filename) as f: config=json.load(f)
        print('2026-11-20T17:00:01.123456789Z INFO kube synthetic',flush=True)
        if mode=='restart':time.sleep(10)
'''

def protocol(data):
    records=[];i=0
    while i<len(data):
        end=data.index(b'\n',i);meta=json.loads(data[i:end]);i=end+1
        length=meta.get('length',0);body=data[i:i+length];i+=length
        if body:meta['payload_hex']=body.hex();meta['payload_bytes']=len(body)
        records.append(meta)
    return records

def observe(name,helper,args,mode='normal',timeout=12):
    env={**os.environ,'PATH':str(BIN)+os.pathsep+os.environ['PATH'],'FIXTURE_CALLS':str(CALLS),'FIXTURE_STATE':str(STATE),'FIXTURE_MODE':mode}
    STATE.unlink(missing_ok=True)
    started=time.monotonic();child=subprocess.Popen(['python3',str(HELPERS/helper),json.dumps(args)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env)
    try: data,err=child.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        child.terminate();data,err=child.communicate(timeout=2)
    records=protocol(data)
    print(json.dumps({'scenario':name,'elapsed_ms':round((time.monotonic()-started)*1000),'exit':child.returncode,'diagnostic_bytes':len(err),'records':records}))
    return records

with tempfile.TemporaryDirectory(prefix='tailpro-phase6-simulation-') as directory:
    BASE=pathlib.Path(directory);BIN=BASE/'bin';BIN.mkdir();CALLS=BASE/'calls.ndjson';STATE=BASE/'state'
    for name in ('journalctl','docker','kubectl'):
        path=BIN/name;path.write_text(STUB);path.chmod(0o700)
    observe('journal-binary-repeated','journal_helper.py',{'unit':'fixture.service'})
    observe('journal-stale-cursor','journal_helper.py',{'unit':'fixture.service','cursor':'stale'})
    observe('journal-frame-cap','journal_helper.py',{'unit':'fixture.service'},'oversize')
    for mode in ('normal','tty','none','denied'):
        observe('docker-'+mode,'docker_capability.py',{'id':'a'*64},mode)
    scope={'action':'probe','kubeconfig':'/synthetic/config','context':'fixture','namespace':'selected','pod':'pod','container':'app'}
    capability=observe('kubernetes-probe','kubernetes_helper.py',scope)[0]
    selected={**scope,**{k:v for k,v in capability.items() if k not in ('kind','length','coverage')},'action':'follow','trusted':True,'previous':False}
    observe('kubernetes-follow','kubernetes_helper.py',selected)
    observe('kubernetes-restart','kubernetes_helper.py',selected,'restart')
    observe('kubernetes-plugin-rejected','kubernetes_helper.py',scope,'plugin')
    observe('kubernetes-denied','kubernetes_helper.py',scope,'denied')
    observe('kubernetes-config-changed','kubernetes_helper.py',{**selected,'config_digest':'0'*64})
    observe('kubernetes-previous','kubernetes_helper.py',{**selected,'previous':True})
    # Run the real file helper against a private synthetic file and stop after one data frame.
    payload=b'first\r\n'+bytes([0,255,226,130,172])+b'partial'
    path=BASE/'bytes.log';path.write_bytes(payload)
    child=subprocess.Popen(['python3',str(HELPERS/'file_helper.py'),json.dumps({'path':str(path)})],stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    output=bytearray()
    deadline=time.monotonic()+2
    while time.monotonic()<deadline:
        if not select.select([child.stdout],[],[],.1)[0]:continue
        output.extend(os.read(child.stdout.fileno(),65536))
        if payload in output:break
    child.terminate();tail,err=child.communicate(timeout=2);output.extend(tail)
    print(json.dumps({'scenario':'real-file-helper-synthetic-bytes','expected_hex':payload.hex(),'records':protocol(output),'diagnostic_bytes':len(err)}))
    calls=[json.loads(line) for line in CALLS.read_text().splitlines()]
    live=[]
    for pid in set(c['pid'] for c in calls):
        try:os.kill(pid,0);live.append(pid)
        except ProcessLookupError:pass
    print(json.dumps({'scenario':'owned-child-cleanup','stub_invocations':len(calls),'remaining_stub_processes':len(live),'real_daemons':False}))
