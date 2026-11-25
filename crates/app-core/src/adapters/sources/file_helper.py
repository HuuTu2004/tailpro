# Bundled read-only program, passed to python3 -c; never installed remotely.
import os, sys, json, stat, time, signal
p = json.loads(sys.argv[1])
path = p['path']
def emit(meta, data=b''):
    meta['length'] = len(data)
    if meta.get('kind') == 'opened': meta['generation'] = str(generation)
    sys.stdout.buffer.write(json.dumps(meta, separators=(',', ':')).encode() + b'\n')
    if data: sys.stdout.buffer.write(data)
    sys.stdout.buffer.flush()
def opened():
    signal.alarm(1)
    fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC | os.O_NOFOLLOW)
    s = os.fstat(fd)
    if not stat.S_ISREG(s.st_mode):
        os.close(fd)
        raise ValueError('regular seekable file required')
    os.lseek(fd, 0, os.SEEK_CUR)
    # Recheck effective access using the descriptor, avoiding pathname substitution.
    check = os.open('/proc/self/fd/' + str(fd), os.O_RDONLY | os.O_NONBLOCK | os.O_CLOEXEC)
    os.close(check)
    signal.alarm(0)
    return fd, s
def guard(fd, offset):
    return os.pread(fd, min(64, offset), max(0, offset-64)).hex()
fd, s = opened()
offset = 0
partial = None
checkpoint = p.get('checkpoint')
generation = int(checkpoint['generation']) if checkpoint else 0
if checkpoint:
    same = str(s.st_dev) == checkpoint['device'] and str(s.st_ino) == checkpoint['inode']
    wanted = int(checkpoint['next_offset'])
    if same and wanted <= s.st_size and guard(fd, wanted) == checkpoint['guard']:
        offset = wanted
        partial = checkpoint.get('partial')
        emit({'kind':'resume', 'offset':str(offset), 'device':str(s.st_dev), 'inode':str(s.st_ino)})
    else:
        generation += 1
        emit({'kind':'truncated' if same else 'rotated', 'uncertain':True})
else:
    offset = max(0, s.st_size - 256*1024)
    if offset:
        # Bounded initial resync. Skip the partial leading physical record.
        prefix = os.pread(fd, min(256*1024, s.st_size-offset), offset)
        newline = prefix.find(b'\n')
        offset = offset + newline + 1 if newline >= 0 else s.st_size
        emit({'kind':'recent_window', 'uncertain':True})
emit({'kind':'opened', 'device':str(s.st_dev), 'inode':str(s.st_ino), 'offset':str(offset), 'guard':guard(fd, offset), 'partial':partial})
last_guard = guard(fd, offset)
while True:
    current = os.fstat(fd)
    if current.st_size < offset or guard(fd, offset) != last_guard:
        offset = 0
        generation += 1
        last_guard = ''
        emit({'kind':'truncated', 'uncertain':True})
        emit({'kind':'opened', 'device':str(current.st_dev), 'inode':str(current.st_ino), 'offset':'0', 'guard':''})
    data = os.pread(fd, 64*1024, offset)
    if data:
        emit({'kind':'data', 'offset':str(offset)}, data)
        offset += len(data)
        last_guard = guard(fd, offset)
        continue
    try:
        named = os.stat(path, follow_symlinks=False)
        if (named.st_dev, named.st_ino) != (current.st_dev, current.st_ino):
            new_fd, new_s = opened()
            generation += 1
            emit({'kind':'rotated', 'uncertain':True})
            os.close(fd)
            fd, s, offset = new_fd, new_s, 0
            emit({'kind':'opened', 'device':str(s.st_dev), 'inode':str(s.st_ino), 'offset':'0', 'guard':''})
            last_guard = ''
            continue
        # Recheck access periodically, without opening nonregular replacements.
        if not os.access('/proc/self/fd/' + str(fd), os.R_OK):
            raise PermissionError('read access revoked')
    except FileNotFoundError:
        pass # Drain old descriptor while waiting for rename replacement.
    emit({'kind':'idle'})
    time.sleep(0.25)
