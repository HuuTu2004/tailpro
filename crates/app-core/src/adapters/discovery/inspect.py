# Read-only, bounded host and directory metadata. Never installed on the host.
import os, sys, json, stat, time, platform, shutil, subprocess, signal

REQUEST = json.loads(sys.argv[1])
ENTRY_CAP = 1000
BYTE_CAP = 256 * 1024
def deadline(*_):
    raise TimeoutError('Metadata deadline reached')
signal.signal(signal.SIGALRM, deadline)
signal.signal(signal.SIGTERM, deadline)
signal.alarm(12)

def send(value):
    data = json.dumps(value, separators=(',', ':')).encode()
    if len(data) > BYTE_CAP:
        raise ValueError('metadata output cap')
    sys.stdout.buffer.write(data + b'\n')
    sys.stdout.buffer.flush()

def revision(path):
    s = os.stat(path, follow_symlinks=False)
    return ':'.join(str(v) for v in (s.st_dev, s.st_ino, s.st_mtime_ns))

def capability(kind, state, detail):
    return {'kind': kind, 'state': state, 'detail': detail}

def check_tool(kind, args):
    if shutil.which(args[0]) is None:
        return capability(kind, 'unavailable', 'Tool is not available in the SSH environment')
    try:
        # Bound memory before reading stdout. Probe needs only success/failure.
        child = subprocess.Popen(args, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            code = child.wait(timeout=3)
        except subprocess.TimeoutExpired:
            child.kill(); child.wait()
            return capability(kind, 'unavailable', 'Provider did not respond before the deadline')
        finally:
            if child.poll() is None:
                child.kill(); child.wait()
        return capability(kind, 'ready' if code == 0 else 'unavailable', 'Metadata probe succeeded; source access is rechecked on Open' if code == 0 else 'Tool exists but the daemon or metadata is not accessible')
    except PermissionError:
        return capability(kind, 'denied', 'Executable access denied')
    except OSError:
        return capability(kind, 'unavailable', 'Tool cannot run in this SSH environment')

def inspect_host():
    system = platform.system()
    import pwd
    account = pwd.getpwuid(os.geteuid())
    home = account.pw_dir
    providers = [capability('file', 'ready', 'Python regular-file reader available'),
                 check_tool('docker', ['docker', 'info', '--format', '{{.ServerVersion}}'])]
    if system == 'Linux':
        providers.append(check_tool('journal', ['systemctl', 'list-units', '--type=service', '--state=running', '--no-legend', '--no-pager', '--plain']) if shutil.which('journalctl') else capability('journal', 'unavailable', 'Journal reader tool is not available'))
    else:
        providers.append(capability('journal', 'unsupported', 'systemd Journal is not supported on this operating system'))
    locations = []
    defaults = [('System logs', '/var/log'), ('PM2 log files', os.path.join(home, '.pm2', 'logs'))]
    if system == 'Darwin':
        defaults.append(('User logs', os.path.join(home, 'Library', 'Logs')))
    for label, path in defaults:
        resolved = os.path.realpath(path)
        if os.path.isdir(resolved) and os.access(resolved, os.R_OK | os.X_OK):
            locations.append({'label': label, 'path': path, 'resolved_path': resolved})
    return {'os': system, 'user': account.pw_name, 'home': os.path.realpath(home), 'helper_ready': True,
            'providers': providers, 'locations': locations, 'issues': []}

def file_metadata(path):
    s = os.stat(path, follow_symlinks=False)
    kind = 'directory' if stat.S_ISDIR(s.st_mode) else 'file' if stat.S_ISREG(s.st_mode) else 'symlink' if stat.S_ISLNK(s.st_mode) else 'unsupported'
    return {'name': os.path.basename(path) or path, 'path': path, 'kind': kind,
            'access': 'ready' if os.access(path, os.R_OK | (os.X_OK if kind == 'directory' else 0)) else 'unknown',
            'size': str(s.st_size) if kind == 'file' else None, 'modified_ns': str(s.st_mtime_ns),
            'identity': str(s.st_dev) + ':' + str(s.st_ino)}

def browse(path):
    path = os.path.abspath(path)
    resolved = os.path.realpath(path)
    result = {'path': path, 'parent': os.path.dirname(path) if os.path.dirname(path) != path else None,
              'resolved_path': resolved if resolved != path else None, 'entries': [], 'complete': True,
              'reasons': [], 'revision': ''}
    if resolved != path:
        result['reasons'] = ['Use the resolved location explicitly before browsing this symbolic link']
        return result
    result['revision'] = revision(path)
    size = 0
    started = time.monotonic()
    with os.scandir(path) as entries:
        for i, entry in enumerate(entries):
            if i >= ENTRY_CAP or size >= BYTE_CAP - 16384 or time.monotonic() - started > 3:
                result['complete'] = False; result['reasons'].append('Directory entry, byte or time cap reached'); break
            try:
                row = file_metadata(entry.path)
                row.pop('identity')
                size += len(json.dumps(row).encode())
                result['entries'].append(row)
            except PermissionError:
                result['reasons'].append('Some directory entries are not accessible')
            except FileNotFoundError:
                result['reasons'].append('Directory changed during listing')
    if result['revision'] != revision(path):
        result['complete'] = False; result['reasons'].append('Directory changed during listing; refresh before paging')
    result['entries'].sort(key=lambda row: (row['kind'] != 'directory', row['name'].casefold()))
    result['reasons'] = sorted(set(result['reasons']))
    return result

try:
    action = REQUEST['action']
    if action == 'inspect':
        send(inspect_host())
    elif action == 'browse':
        send(browse(REQUEST['path']))
    elif action == 'revision':
        send({'revision': revision(REQUEST['path'])})
    elif action == 'file':
        send(file_metadata(REQUEST['path']))
    else:
        raise ValueError('unsupported metadata operation')
except PermissionError:
    send({'error': 'Permission denied for this location', 'code': 'denied'})
except FileNotFoundError:
    send({'error': 'Location does not exist', 'code': 'missing'})
except (OSError, ValueError, ImportError):
    send({'error': 'Host metadata operation is unavailable or outside its bounds', 'code': 'unavailable'})
