"""Collect dependency license texts for the Windows preview package (no source/media uploads)."""
import json,sys
from pathlib import Path

metadata=Path(sys.argv[1]);output=Path(sys.argv[2])
data=json.loads(metadata.read_text(encoding='utf-8-sig'))
parts=['AI Reaction Studio third-party Rust dependency license texts\n']
seen=set()
for package in sorted(data['packages'],key=lambda p:(p['name'],p['version'])):
    if not package.get('source'):continue
    parts.append(f"\n{'='*72}\n{package['name']} {package['version']}\nLicense: {package.get('license')}\nSource: {package.get('repository') or package.get('source')}\n")
    root=Path(package['manifest_path']).parent
    candidates=[]
    if package.get('license_file'):candidates.append(root/package['license_file'])
    for folder in [root,*list(root.parents)[:2]]:
        if folder.exists():
            candidates.extend(p for p in folder.iterdir() if p.is_file() and p.name.upper().startswith(('LICENSE','LICENCE','COPYING','NOTICE','OFL')))
    for folder in [root/'fonts',root/'src/fonts']:
        if folder.exists():
            candidates.extend(p for p in folder.rglob('*') if p.is_file() and ('LICENSE' in p.name.upper() or p.name.upper()=='OFL.TXT'))
    for p in sorted(set(candidates)):
        if p.suffix.lower() in ['.rs','.toml','.json'] or not p.exists() or p.stat().st_size>1_000_000:continue
        text=p.read_text(encoding='utf-8',errors='replace')
        key=(p.name,text)
        if key in seen:parts.append(f'\n{p.name}: identical license text included above.\n');continue
        seen.add(key);parts.append('\n'+p.name+'\n'+text+'\n')
output.parent.mkdir(parents=True,exist_ok=True)
output.write_text(''.join(parts),encoding='utf-8')
print(f'Collected {len(seen)} distinct license texts into {output.name}')
