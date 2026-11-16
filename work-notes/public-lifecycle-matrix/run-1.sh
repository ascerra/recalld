set -eu
cd /source
cargo test --offline --all-features --no-run --message-format=json > /validation/build-1.jsonl 2> /validation/build-1.log
python3 - <<'PYINNER'
import json,pathlib,hashlib
p=pathlib.Path('/validation');entries=[json.loads(l) for l in (p/'build-1.jsonl').read_text().splitlines() if l.startswith('{')]
names=sorted(set(e['executable'] for e in entries if e.get('reason')=='compiler-artifact' and e.get('executable')))
assert names
out=[]
for name in names:
 f=pathlib.Path(name);out.append({'path':name,'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'size':f.stat().st_size,'mtime_ns':f.stat().st_mtime_ns})
(p/'executables-1.json').write_text(json.dumps(out,indent=2))
PYINNER
cargo test --offline --all-features public_matrix_ -- --nocapture > /validation/focus-1.log 2>&1
