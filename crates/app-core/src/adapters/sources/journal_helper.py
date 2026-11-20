# Bundled read-only adapter. No service control, shell, cursor files or remote install.
import sys, json, subprocess, selectors, os, signal, time
p = json.loads(sys.argv[1])
CAP = 128 * 1024
child = None

def send(v):
    print(json.dumps(v, separators=(',', ':'), ensure_ascii=False), flush=True)

def close():
    global child
    if child is not None:
        child.terminate()
        try: child.wait(timeout=.3)
        except subprocess.TimeoutExpired:
            child.kill(); child.wait()
        child = None

def stop(*_):
    close(); sys.exit(0)
signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGHUP, stop)
base = ['journalctl', '--system', '--no-pager', '--output=json',
        '--output-fields=MESSAGE,PRIORITY,_TRANSPORT', '--unit='+p['unit']]

def launch(extra):
    global child
    child = subprocess.Popen(base + extra, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             stdin=subprocess.DEVNULL, env={**os.environ, 'LC_ALL':'C', 'SYSTEMD_COLORS':'0'})
    sel = selectors.DefaultSelector()
    sel.register(child.stdout, selectors.EVENT_READ, 'out')
    sel.register(child.stderr, selectors.EVENT_READ, 'err')
    return sel

def lines(sel, seconds=None):
    pending = bytearray()
    until = time.monotonic() + seconds if seconds else None
    while sel.get_map():
        if until and time.monotonic() >= until: raise TimeoutError()
        for key, _ in sel.select(.25):
            block = os.read(key.fileobj.fileno(), 8192)
            if not block:
                sel.unregister(key.fileobj); continue
            if key.data == 'err':
                # Permission notices can coexist with exit 0 and partial output.
                send({'kind':'access', 'status':'restricted_or_unavailable'})
                continue
            pending.extend(block)
            while b'\n' in pending:
                line, _, rest = pending.partition(b'\n'); pending = bytearray(rest)
                if len(line) > CAP: raise ValueError()
                yield json.loads(line)
            if len(pending) > CAP: raise ValueError()
    if pending: raise ValueError()
try:
    cursor = p.get('cursor')
    if cursor:
        # Check the first record at the cursor. A vacuumed cursor may seek to a
        # different record without a nonzero exit; never claim successful resume.
        sel = launch(['--cursor='+cursor, '--lines=+1'])
        try:
            first = next(lines(sel, 1.0), None)
            valid = first is not None and first.get('__CURSOR') == cursor
        except (TimeoutError, ValueError, json.JSONDecodeError): valid = False
        finally: sel.close(); close()
        if not valid:
            send({'kind':'gap', 'status':'cursor_invalid_or_vacuumed; bounded recent fallback'})
            cursor = None
    sel = launch(['--follow', '--no-tail', '--after-cursor='+cursor] if cursor else ['--follow', '--lines=200'])
    send({'kind':'opened', 'status':'visible_entries_only; permissions not proven; RAM cursor only'})
    for item in lines(sel):
        cursor = item.get('__CURSOR')
        if not isinstance(cursor, str) or len(cursor) > 2048: raise ValueError()
        message = item.get('MESSAGE')
        if not isinstance(message, str):
            send({'kind':'record', 'cursor':cursor, 'message':'[unsupported binary or repeated Journal MESSAGE]', 'uncertain':True})
        else:
            send({'kind':'record', 'cursor':cursor, 'message':message,
                  'time_us':item.get('__REALTIME_TIMESTAMP'), 'priority':item.get('PRIORITY'),
                  'uncertain':False})
    send({'kind':'unavailable', 'status':'journal ended; check journalctl/systemd/permissions'})
except FileNotFoundError:
    send({'kind':'unsupported', 'status':'journalctl unavailable; Python 3 and systemd journal required'})
except (ValueError, OSError, TimeoutError):
    send({'kind':'unavailable', 'status':'Journal frame/access failure; continuity unknown'})
finally:
    close()
