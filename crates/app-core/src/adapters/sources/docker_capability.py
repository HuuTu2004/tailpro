# Read-only capability preflight. Emits no inspect JSON, paths, options or diagnostics.
import json, subprocess, sys, selectors, os, time
p=json.loads(sys.argv[1]); child=None
try:
    child=subprocess.Popen(['docker','inspect','--type=container',p['id']],stdout=subprocess.PIPE,stderr=subprocess.PIPE,stdin=subprocess.DEVNULL)
    sel=selectors.DefaultSelector();sel.register(child.stdout,selectors.EVENT_READ,'out');sel.register(child.stderr,selectors.EVENT_READ,'err');data=bytearray();size=0;deadline=time.monotonic()+3
    while sel.get_map():
        if time.monotonic()>deadline: raise ValueError()
        for key,_ in sel.select(.1):
            block=os.read(key.fileobj.fileno(),8192)
            if not block: sel.unregister(key.fileobj);continue
            size+=len(block)
            if size>128*1024: raise ValueError()
            if key.data=='out': data.extend(block)
    if child.wait(timeout=.3)!=0: raise ValueError()
    item=json.loads(data)[0]
    if item.get('Id')!=p['id']: raise ValueError()
    driver=item.get('HostConfig',{}).get('LogConfig',{}).get('Type','')
    print(json.dumps({'kind':'capability','driver':driver if driver in ('json-file','local','none','journald') else 'other_or_unknown','tty':bool(item.get('Config',{}).get('Tty')), 'readable':driver!='none', 'coverage':'driver cache/history unknown; timestamp overlap replay, no durable offset'}))
except (ValueError,KeyError,IndexError,OSError,subprocess.SubprocessError):
    print(json.dumps({'kind':'needs_action','reason':'Docker inspect unavailable, denied or capability response capped'}))
finally:
    if child and child.poll() is None:
        child.terminate()
        try: child.wait(timeout=.3)
        except subprocess.TimeoutExpired: child.kill();child.wait()
