# Static, deliberately restricted Compose reader; never invokes Compose or env.
import sys, json, os, stat, re, hashlib
from urllib.parse import urlsplit
p = json.loads(sys.argv[1])
nodes, edges, reasons = {}, [], set()
endpoint_bytes = 0
fingerprints = []
total = 0
names = set(p.get('endpoint_names', []))
valid = lambda s: isinstance(s, str) and re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,127}', s)

def read(path):
    global total
    root = next((r for r in p['roots'] if os.path.commonpath([path, r]) == r), None)
    if root is None: raise ValueError()
    # Walk from / with directory descriptors; no parent symlink or FIFO races.
    fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    try:
        parts = path.strip('/').split('/')
        for part in parts[:-1]:
            nxt = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            os.close(fd); fd = nxt
        f = os.open(parts[-1], os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW, dir_fd=fd)
        try:
            st = os.fstat(f)
            if not stat.S_ISREG(st.st_mode) or st.st_size > 65536: raise ValueError()
            fingerprints.append([path,str(st.st_dev),str(st.st_ino),str(st.st_size),str(st.st_mtime_ns),str(st.st_ctime_ns)])
            if p.get('metadata_only'): return ''
            data = bytearray()
            while len(data) <= 65536:
                b = os.read(f, min(8192, 65537-len(data)))
                if not b: break
                data.extend(b)
            total += len(data)
            if len(data) > 65536 or total > 262144: raise ValueError()
            return data.decode('utf-8')
        finally: os.close(f)
    finally: os.close(fd)

def scalar(s):
    s = s.strip()
    if s.startswith('"'):
        try: return json.loads(s)
        except Exception: return None
    if s.startswith("'") and s.endswith("'"): return s[1:-1].replace("''", "'")
    if any(c in s for c in '#{}&*!|>'): return None
    return s

def subset(text):
    # Parse only block services, depends_on and selected literal environment.
    # Everything executable/interpolated/imported remains unresolved.
    services, service, section = {}, None, None
    in_services = False
    for line in text.splitlines():
        if len(line) > 4096 or '\t' in line: raise ValueError()
        stripped = line.strip()
        if not stripped or stripped.startswith('#'): continue
        indent = len(line)-len(line.lstrip(' '))
        if any(x in stripped for x in ['${', '$$', '&', '*', '<<:']): reasons.add('variables/anchors unresolved; static subset only')
        if indent == 0:
            in_services = stripped == 'services:'
            service, section = None, None
            if stripped.startswith(('include:', 'extends:')): reasons.add('Compose includes/extends unresolved; explicitly approve each file')
            if stripped.startswith('services:') and not in_services: raise ValueError()
            continue
        if not in_services: continue
        if indent == 2:
            name = scalar(stripped[:-1]) if stripped.endswith(':') else None
            if not valid(name): raise ValueError()
            if name in services or len(services) >= 256: raise ValueError()
            service = name; services[name] = {'depends_on':[], 'environment':{}}; section = None
        elif service and indent == 4:
            section = None
            key, sep, rest = stripped.partition(':')
            if not sep: raise ValueError()
            if key in ('extends','include'): reasons.add('Compose includes/extends unresolved')
            if key in ('depends_on','environment'):
                section = key
                if rest.strip():
                    if key == 'depends_on' and rest.strip().startswith('[') and rest.strip().endswith(']'):
                        services[service][key] = [scalar(x) for x in rest.strip()[1:-1].split(',') if x.strip()]
                    else: reasons.add('unsupported inline Compose metadata; partial')
        elif service and section == 'depends_on' and indent == 6:
            dep = scalar(stripped[2:]) if stripped.startswith('- ') else scalar(stripped[:-1]) if stripped.endswith(':') else None
            if valid(dep): services[service]['depends_on'].append(dep)
            else: reasons.add('unresolved dependency')
        elif service and section == 'environment' and indent == 6:
            if stripped.startswith('- '): key, sep, value = stripped[2:].partition('=')
            else: key, sep, value = stripped.partition(':')
            key = key.strip()
            if key in names:
                value = scalar(value) if sep else None
                services[service]['environment'][key] = value
    return {'services':services}

pending_endpoints = []
for path in p['files']:
    try:
        text = read(path)
        if p.get('metadata_only'): continue
        if text.lstrip().startswith('{'):
            # Preflight depth before JSON tree allocation.
            depth = 0; quoted = False; escaped = False
            for c in text:
                if quoted:
                    if escaped: escaped = False
                    elif c == '\\': escaped = True
                    elif c == '"': quoted = False
                elif c == '"': quoted = True
                elif c in '[{':
                    depth += 1
                    if depth > 32: raise ValueError()
                elif c in ']}': depth -= 1
            def unique(pairs):
                result = {}
                for key, value in pairs:
                    if key in result: raise ValueError()
                    result[key] = value
                return result
            doc = json.loads(text, object_pairs_hook=unique)
        else: doc = subset(text)
        if doc.get('include'): reasons.add('includes unresolved; no automatic file expansion')
        services = doc.get('services', {})
        if not isinstance(services, dict) or len(services) > 256: raise ValueError()
        for name, spec in services.items():
            if not valid(name) or not isinstance(spec, dict): reasons.add('unsupported service metadata'); continue
            if len(nodes) >= 256 and name not in nodes: reasons.add('node cap'); break
            if name in nodes: reasons.add('duplicate service across files; merge unresolved')
            nodes[name] = {'id':name, 'sources':[], 'observed_errors':None, 'provenance':'configured service in '+path}
            if spec.get('extends'): reasons.add('extends unresolved')
            deps = spec.get('depends_on', [])
            if not isinstance(deps, (list,dict)): reasons.add('unresolved dependency'); continue
            for dep in deps:
                if valid(dep) and len(edges) < 512: edges.append({'from':name,'to':dep,'kind':'configured','provenance':'static depends_on in '+path+'; no traffic evidence'})
                else: reasons.add('edge cap or unresolved dependency')
            env = spec.get('environment', {})
            if isinstance(env, list): env = dict(x.split('=',1) for x in env if isinstance(x,str) and '=' in x)
            if not isinstance(env, dict): reasons.add('unsupported endpoint provider'); continue
            for key in names:
                if key not in env: continue
                value = env[key]
                endpoint_bytes += len(value.encode('utf-8')) if isinstance(value,str) else 0
                if endpoint_bytes > 16384: reasons.add('endpoint byte cap'); continue
                if not isinstance(value,str) or len(value)>1024 or '$' in value:
                    reasons.add('endpoint unresolved; runtime environment is not read'); continue
                # Only hostname is retained. Credentials, paths, queries and values
                # never cross the SSH boundary or enter the graph.
                try: host = urlsplit(value).hostname
                except ValueError: host = None
                if valid(host): pending_endpoints.append((name,host,path))
                else: reasons.add('endpoint hint unsupported')
    except (OSError, ValueError, TypeError, RecursionError):
        reasons.add('config denied/unsupported/capped; static subset only')
for source, host, path in pending_endpoints[:512]:
    if host in nodes and len(edges)<512: edges.append({'from':source,'to':host,'kind':'inferred','provenance':'allowlisted literal config endpoint hostname in '+path+'; no runtime traffic evidence'})
    else: reasons.add('endpoint outside configured scope or edge cap')
if any(e['from'] not in nodes or e['to'] not in nodes for e in edges): reasons.add('dependency outside configured scope')
edges = [e for e in edges if e['from'] in nodes and e['to'] in nodes]
if len(pending_endpoints)>512: reasons.add('endpoint cap')
result = {'schema_version':2,'nodes':list(nodes.values()),'edges':edges,'complete':not reasons,
          'reasons':sorted(reasons),'coverage':'static configured/inferred dependencies only; selected config literals only; no runtime env/.env/interpolation/traffic; counts are retained observations, no rates', 'config_digest':'', 'config_revision':hashlib.sha256(json.dumps(fingerprints,separators=(',',':')).encode()).hexdigest()}
result['config_digest'] = hashlib.sha256(json.dumps(result,sort_keys=True).encode()).hexdigest()
encoded = json.dumps(result,separators=(',',':'))
if len(encoded)>262144: raise ValueError('graph cap')
print(encoded,flush=True)
