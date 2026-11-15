set -eu
cd /source
cargo fmt --check > /validation/evidence/decoder-final-fmt.log 2>&1
cargo test --offline --lib --no-run --message-format=json > /validation/evidence/decoder-final-build.jsonl 2> /validation/evidence/decoder-final-build.log
python3 - <<'PY'
import json,pathlib,hashlib
root=pathlib.Path('/validation/evidence');items=[json.loads(x) for x in (root/'decoder-final-build.jsonl').read_text().splitlines() if x.startswith('{')]
paths=[a['executable'] for a in items if a.get('reason')=='compiler-artifact' and a.get('profile',{}).get('test') and a.get('executable')];assert len(paths)==1
p=pathlib.Path(paths[0]);(root/'decoder-final-executable.json').write_text(json.dumps({'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'size':p.stat().st_size,'mtime_ns':p.stat().st_mtime_ns},indent=2));(root/'decoder-final-executable-path').write_text(str(p))
PY
binary=$(cat /validation/evidence/decoder-final-executable-path)
"$binary" decoder_native_ --nocapture > /validation/evidence/decoder-final-tests.log 2>&1
"$binary" native_lifecycle_wire --nocapture > /validation/evidence/decoder-final-protocol.log 2>&1
"$binary" legacy_api2_ --nocapture > /validation/evidence/decoder-final-legacy.log 2>&1
cargo clippy --offline --all-targets > /validation/evidence/decoder-final-clippy.log 2>&1
