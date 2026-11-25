# Metadata-only unless exact config capability is explicitly supplied.
import os, sys, stat, json, time, signal, shlex, subprocess, selectors, re, fnmatch, platform, shutil
p = json.loads(sys.argv[1])
def interrupted(signum, frame):
    raise TimeoutError('bounded probe interrupted')
for sig in (signal.SIGALRM, signal.SIGHUP, signal.SIGTERM): signal.signal(sig,interrupted)
signal.alarm(15)
started = time.monotonic()
entries = 0
output = 0
candidates = 0
reasons = set()
seen = set()
identities = set()
config_bytes = 0
config_seen = set()
observations = {}
def send(obj):
    global output
    data = json.dumps(obj, separators=(',', ':')).encode() + b'\n'
    if output + len(data) > 1024*1024:
        reasons.add('output cap'); return False
    output += len(data)
    sys.stdout.buffer.write(data); sys.stdout.buffer.flush()
    return True
def capped():
    return candidates >= 500 or entries >= (200 if p.get('complete') else 1000) or time.monotonic()-started > (0.9 if p.get('complete') else 14)
def candidate(path, provider, provenance, tag=None):
    global candidates
    if candidates >= (20 if p.get('complete') else 500):
        reasons.add('candidate cap'); return
    if path in seen: return
    seen.add(path)
    access = 'unknown'; size = None; mtime = None; identity = None
    try:
        s = os.stat(path, follow_symlinks=False)
        if not stat.S_ISREG(s.st_mode): return
        identity = str(s.st_dev)+':'+str(s.st_ino)
        if identity in identities: return
        identities.add(identity)
        access = 'ready' if os.access(path, os.R_OK) else 'unknown'
        size = str(s.st_size); mtime = str(s.st_mtime_ns)
        if len(observations) < 20: observations[path] = (s.st_dev, s.st_ino, s.st_size, time.monotonic())
    except PermissionError: access = 'denied'
    except FileNotFoundError: access = 'missing'
    except OSError: pass
    candidates += 1
    compressed = path.lower().endswith(('.gz','.bz2','.xz','.zip','.zst','.lz4'))
    archive = compressed or bool(re.search(r'(?:\.\d+(?:\.log)?$|\d{4}[-_]\d{2}[-_]\d{2}|/archive(?:s)?/)', path))
    tags = ([tag] if tag else []) + (['archive_hint'] if archive else [])
    if compressed: access = 'unsupported'
    send({'file_type':'compressed_archive' if compressed else 'regular_file','confidence':'configured_hint' if provider in ('nginx','caddy','supervisor','systemd') else 'metadata','process_hint':provenance if provider=='process_fd' else None,'identity':identity,'path':path, 'provider':provider, 'provenance':provenance, 'access':access, 'size':size, 'mtime':mtime, 'tags':tags, 'activity':'Opened' if provider == 'process_fd' else ('Recently modified' if mtime and 0 <= time.time()-int(mtime)/1e9 <= 7*86400 else 'Unknown')})
def scan(root, depth, provider, prefix=''):
    global entries
    if capped(): reasons.add('entry/deadline cap'); return
    if os.path.realpath(root) != os.path.abspath(root): reasons.add('symlink root requires explicit resolved scope'); return
    scan_started = time.monotonic()
    try:
        with os.scandir(root) as iterator:
            while not capped() and time.monotonic()-scan_started < 5:
                try: entry = next(iterator)
                except StopIteration: return
                entries += 1
                if prefix and not entry.name.startswith(prefix): continue
                try:
                    if entry.is_file(follow_symlinks=False): candidate(entry.path, provider, 'regular-file metadata')
                    elif depth and entry.is_dir(follow_symlinks=False): scan(entry.path, depth-1, provider)
                except PermissionError: reasons.add('permission denied')
            reasons.add('entry/deadline cap')
    except PermissionError: reasons.add('permission denied: '+root)
    except FileNotFoundError: reasons.add('missing root: '+root)
    except OSError: reasons.add('unsupported root: '+root)
def command(args, limit=256*1024):
    # No shell; bounded stdout/stderr, deadline, and owned process cleanup.
    try: child = subprocess.Popen(args, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    except FileNotFoundError: reasons.add('unsupported provider: '+args[0]); return b''
    data = bytearray(); until = time.monotonic()+4
    selector = selectors.DefaultSelector(); selector.register(child.stdout, selectors.EVENT_READ)
    try:
        while len(data) < limit and time.monotonic() < until:
            if not selector.select(0.1):
                if child.poll() is not None: break
                continue
            chunk = os.read(child.stdout.fileno(), min(4096, limit-len(data)))
            if not chunk: break
            data.extend(chunk)
        if len(data) >= limit: reasons.add('provider output cap')
        if time.monotonic() >= until: reasons.add('provider timeout')
    finally:
        try: child.wait(timeout=0.2)
        except subprocess.TimeoutExpired: child.kill(); child.wait()
        child.stdout.close(); selector.close()
    if child.returncode != 0: reasons.add('provider denied/unavailable: '+args[0])
    return bytes(data)
def configured(path, provider):
    if not path.startswith('/') or any(c in path for c in '$%{}'):
        reasons.add(provider+': unresolved configured hint'); return
    candidate(path, provider, 'configured_hint; on-disk config may differ from loaded config', provider)
def includes(pattern, depth):
    global entries
    parent, name = os.path.split(pattern)
    if not os.path.isabs(pattern) or any(c in parent for c in '*?$%'):
        reasons.add('unresolved include'); return
    resolved=os.path.realpath(parent)
    roots=[os.path.realpath(r) for r in p.get('config_roots',[])]
    if not any(os.path.commonpath([resolved,r])==r for r in roots): reasons.add('include outside approved roots'); return
    if any(c in name for c in '*?['):
        try:
            with os.scandir(parent) as it:
                while entries<1000 and len(config_seen)<32 and not capped():
                    try: entry=next(it)
                    except StopIteration: return
                    entries+=1
                    if fnmatch.fnmatchcase(entry.name,name):config(entry.path,depth)
                reasons.add('include entry/file cap')
        except OSError:reasons.add('include unavailable')
    else:config(pattern,depth)
def config(path, depth=0):
    global config_bytes
    resolved = os.path.realpath(path)
    roots = [os.path.realpath(r) for r in p.get('config_roots', [])]
    if not any(os.path.commonpath([resolved, r]) == r for r in roots): reasons.add('config outside approved roots'); return
    if resolved in config_seen: return
    if depth > 4 or len(config_seen) >= 32: reasons.add('config include cap'); return
    config_seen.add(resolved)
    try:
        fd = os.open(resolved, os.O_RDONLY|os.O_NONBLOCK|os.O_NOFOLLOW|os.O_CLOEXEC)
        try:
            s = os.fstat(fd)
            if not stat.S_ISREG(s.st_mode) or s.st_size > 64*1024 or config_bytes+s.st_size > 256*1024: reasons.add('config byte/type cap'); return
            content = os.read(fd, min(64*1024+1, 256*1024-config_bytes+1))
            config_bytes += len(content)
            if len(content) > 64*1024 or config_bytes > 256*1024: reasons.add('config byte cap'); return
            text = content.decode('utf-8')
        finally: os.close(fd)
    except (OSError, UnicodeError): reasons.add('config unavailable'); return
    name = os.path.basename(path).lower()
    if 'caddy' in name:
        try:
            obj = json.loads(text)
            def walk(obj, depth=0):
                if depth > 32: return
                if isinstance(obj, dict):
                    if obj.get('writer') == 'file' and isinstance(obj.get('filename'), str): configured(obj['filename'], 'caddy')
                    for v in obj.values(): walk(v, depth+1)
                elif isinstance(obj, list):
                    for v in obj[:200]: walk(v, depth+1)
            walk(obj)
        except (ValueError, RecursionError):
            for line in text.splitlines():
                try: tokens = shlex.split(line, comments=True)
                except ValueError: continue
                if tokens[:2] == ['output', 'file'] and len(tokens) >= 3: configured(tokens[2], 'caddy')
                if tokens[:1] == ['import']: reasons.add('caddy import unresolved')
    elif 'supervisor' in path or any('supervisor' in r for r in p.get('config_roots',[])):
        for line in text.splitlines():
            match = re.match(r'\s*(stdout_logfile|stderr_logfile)\s*=\s*(.*?)\s*$', line)
            if match and match[2] not in ('AUTO','NONE'): configured(match[2], 'supervisor')
        section=''
        for line in text.splitlines():
            stripped=line.strip()
            if stripped.startswith('[') and stripped.endswith(']'): section=stripped
            if section=='[include]' and stripped.startswith('files') and '=' in stripped:
                for inc in stripped.split('=',1)[1].split():
                    if '%' in inc:reasons.add('supervisor dynamic include unresolved')
                    else:includes(inc if inc.startswith('/') else os.path.join(os.path.dirname(path),inc),depth+1)
    elif 'nginx' in path or any('nginx' in r for r in p.get('config_roots',[])):
        for line in text.splitlines():
            try: tokens = shlex.split(line.strip().rstrip(';'), comments=True)
            except ValueError: continue
            if len(tokens) >= 2 and tokens[0] in ('access_log','error_log') and tokens[1] not in ('off','stderr') and not tokens[1].startswith('syslog:'): configured(tokens[1], 'nginx')
            if len(tokens) >= 2 and tokens[0] == 'include':
                inc = tokens[1]
                if any(c in inc for c in '$%'): reasons.add('nginx dynamic include unresolved')
                else: includes(inc if inc.startswith('/') else os.path.join(os.path.dirname(path),inc), depth+1)
    else: reasons.add('unsupported config provider')
def cover(provider, location, operation):
    previous = set(reasons)
    operation()
    issues = sorted(reasons - previous)
    state = 'denied' if any('permission denied' in issue for issue in issues) else 'partial' if issues else 'complete'
    send({'coverage': {'provider': provider, 'location': location, 'state': state, 'detail': '; '.join(issues)}})

def containers():
    global candidates
    fmt = '{{.ID}}\t{{.Names}}\t{{.Label "com.docker.compose.project"}}\t{{.Label "com.docker.compose.service"}}'
    for line in command(['docker','ps','--last','500','--filter','status=running','--no-trunc','--format',fmt]).decode('utf-8','replace').splitlines()[:500]:
        fields = line.split('\t')
        if len(fields) != 4 or not re.fullmatch('[a-fA-F0-9]{64}',fields[0]): continue
        if candidates >= 500: reasons.add('candidate cap'); break
        candidates += 1
        send({'file_type':'container','confidence':'metadata','identity':fields[0],'path':fields[1], 'provider':'docker', 'provenance':'Docker running-container metadata', 'container_id':fields[0], 'access':'ready', 'tags':[x for x in fields[2:] if x], 'activity':'Unknown'})

def units():
    for line in command(['systemctl','list-units','--type=service','--state=running','--no-legend','--no-pager','--plain'], 64*1024).decode('utf-8','replace').splitlines()[:200]:
        unit = line.split()[0] if line.split() else ''
        if unit: send({'unit_hint':unit})
    for unit in p.get('selected_units', [])[:32]:
        text = command(['systemctl','show',unit,'--no-pager','--property=StandardOutput,StandardError,FragmentPath,DropInPaths'], 64*1024).decode('utf-8','replace')
        for line in text.splitlines():
            if line.startswith(('StandardOutput=','StandardError=')):
                value = line.split('=',1)[1]
                for prefix in ('file:','append:','truncate:'):
                    if value.startswith(prefix): configured(value[len(prefix):], 'systemd')

if p.get('complete'):
    scan(p['parent'], 0, 'completion', p['prefix'])
else:
    if p.get('docker'):
        if shutil.which('docker'): cover('docker', 'Running containers', containers)
        else: send({'coverage': {'provider':'docker','location':'Running containers','state':'unavailable','detail':'Docker is not available in the SSH environment'}})
    if p.get('journal') or p.get('selected_units'):
        if platform.system() == 'Linux' and shutil.which('systemctl'):
            cover('journal', 'Running services', units)
        else: send({'coverage': {'provider':'journal','location':'Running services','state':'unsupported','detail':'systemd is not supported on this host'}})
    for pid in p.get('selected_pids', [])[:32]:
        if platform.system() != 'Linux':
            send({'coverage': {'provider':'process_fd','location':str(pid),'state':'unsupported','detail':'Process FD discovery requires Linux proc metadata'}}); continue
        try:
            with os.scandir('/proc/'+str(pid)+'/fd') as it:
                for n in range(64):
                    if entries >= 1000: reasons.add('FD entry cap'); break
                    entries += 1
                    try: fd = next(it)
                    except StopIteration: break
                    try:
                        target = os.readlink(fd.path)
                        if target.endswith(('.log','.out','.err')) and stat.S_ISREG(os.stat(fd.path).st_mode): candidate(target,'process_fd','selected PID '+str(pid)+' open descriptor')
                    except OSError: pass
        except OSError: reasons.add('PID FD metadata unavailable')
    if p.get('var_log'):
        root = os.path.realpath('/var/log')
        cover('var_log', root, lambda: scan(root,1,'var_log'))
    for root in p.get('directories', []):
        cover('directory', root, lambda root=root: scan(root,1,'directory'))
    for root in p.get('project_roots', []):
        # Backwards-compatible presets are optional hints, not proof of a framework.
        for suffix in ('', 'logs','log','storage/logs','tmp/logs'):
            location = os.path.join(root,suffix) if suffix else root
            if os.path.isdir(location): cover('project', location, lambda location=location: scan(location,1,'project'))
    if p.get('pm2_root'): cover('pm2_directory', p['pm2_root'], lambda: scan(p['pm2_root'],0,'pm2_directory'))
    if p.get('config_consent'):
        for file in p.get('config_files', [])[:32]: cover('config', file, lambda file=file: config(file))
if observations and not p.get('complete') and time.monotonic()-started < 12:
    time.sleep(1)
    for path, (device,inode,size,observed) in observations.items():
        try:
            s=os.stat(path,follow_symlinks=False)
            if (s.st_dev,s.st_ino)==(device,inode) and s.st_size>size:
                send({'growth':path,'window_ms':str(round((time.monotonic()-observed)*1000))})
        except OSError: pass
if capped(): reasons.add('entry/deadline cap')
send({'done':True, 'reasons':sorted(reasons)[:200]})
