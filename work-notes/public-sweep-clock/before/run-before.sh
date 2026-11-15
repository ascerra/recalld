set -eu
cd /source
cargo clean --offline -p recalld > /validation/package-clean.log 2>&1
cargo test --offline --all-features --no-run --message-format=json > /validation/before-build.jsonl 2> /validation/before-build.log
python3 - <<'PYINNER'
import json,pathlib,hashlib
p=pathlib.Path('/validation');entries=[json.loads(l) for l in (p/'before-build.jsonl').read_text().splitlines() if l.startswith('{')]
names=sorted(set(e['executable'] for e in entries if e.get('reason')=='compiler-artifact' and e.get('executable')));assert names
out=[]
for name in names:
 f=pathlib.Path(name);out.append({'path':name,'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'size':f.stat().st_size,'mtime_ns':f.stat().st_mtime_ns})
(p/'before-executables.json').write_text(json.dumps(out,indent=2))
PYINNER
cargo test --offline --all-features public_sweep_clock_ -- --nocapture > /validation/before-tests.log 2>&1
