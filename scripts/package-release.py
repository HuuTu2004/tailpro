"""Collect native bundles and a matching MCP executable; never include user settings."""
import json, os, pathlib, shutil, sys, zipfile
root = pathlib.Path(__file__).resolve().parent.parent
label = sys.argv[1]
version = json.loads((root / 'src-tauri/tauri.conf.json').read_text())['version']
out = root / 'release-assets'
out.mkdir(exist_ok=True)
release = root / 'target/release'
required = {'.exe'} if label.startswith('windows') else {'.dmg'} if label.startswith('macos') else {'.deb', '.AppImage'}
found = set()
for source in (release / 'bundle').rglob('*'):
    if source.is_file() and source.suffix in required:
        shutil.copy2(source, out / source.name)
        found.add(source.suffix)
if found != required:
    raise SystemExit(f'Missing native bundle types: {required - found}')
exe = 'tailpro-mcp.exe' if os.name == 'nt' else 'tailpro-mcp'
with zipfile.ZipFile(out / f'tailpro-mcp_{version}_{label}.zip', 'w', zipfile.ZIP_DEFLATED) as archive:
    archive.write(release / exe, exe)
    for name in ('LICENSE', 'README.md', 'README.vi.md'):
        archive.write(root / name, name)
print('\n'.join(path.name for path in sorted(out.iterdir())))
