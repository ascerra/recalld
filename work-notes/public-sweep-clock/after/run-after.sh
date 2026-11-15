set -eu
cd /source
cargo test --offline --all-features --no-run --message-format=json > /validation/after-build.jsonl 2> /validation/after-build.log
python3 - <<'PYINNER'
import json,pathlib,hashlib
p=pathlib.Path('/validation');entries=[json.loads(l) for l in (p/'after-build.jsonl').read_text().splitlines() if l.startswith('{')]
names=sorted(set(e['executable'] for e in entries if e.get('reason')=='compiler-artifact' and e.get('executable')));assert names
out=[]
for name in names:
 f=pathlib.Path(name);out.append({'path':name,'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'size':f.stat().st_size,'mtime_ns':f.stat().st_mtime_ns})
(p/'after-executables.json').write_text(json.dumps(out,indent=2))
PYINNER
cargo test --offline --all-features public_sweep_clock_ -- --nocapture > /validation/after-focused.log 2>&1
cargo test --offline --all-features manual_origin_ -- --nocapture > /validation/after-origin.log 2>&1
cargo test --offline --all-features native_sweep_compaction_ -- --nocapture > /validation/after-compaction.log 2>&1
cargo test --offline --all-features actual_manual_sweep_ -- --nocapture > /validation/after-manual.log 2>&1
cargo test --offline --all-features > /validation/after-full.log 2>&1
cargo fmt --all -- --check > /validation/after-fmt.log 2>&1
cargo clippy --offline --all-targets --all-features > /validation/after-clippy.log 2>&1
