set -eu
cd /source
cargo test --offline --all-features --no-run --message-format=json > /validation/build-3.jsonl 2> /validation/build-3.log
python3 - <<'PYINNER'
import json,pathlib,hashlib
p=pathlib.Path('/validation');entries=[json.loads(l) for l in (p/'build-3.jsonl').read_text().splitlines() if l.startswith('{')]
names=sorted(set(e['executable'] for e in entries if e.get('reason')=='compiler-artifact' and e.get('executable')))
assert names
out=[]
for name in names:
 f=pathlib.Path(name);out.append({'path':name,'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'size':f.stat().st_size,'mtime_ns':f.stat().st_mtime_ns})
(p/'executables-3.json').write_text(json.dumps(out,indent=2))
PYINNER
cargo test --offline --all-features public_matrix_summary_ -- --nocapture > /validation/focus-3.log 2>&1
cargo fmt --all -- --check > /validation/fmt-3.log 2>&1
cargo clippy --offline --all-targets --all-features > /validation/clippy-3.log 2>&1
