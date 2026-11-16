set -eu
cd /source
cargo fmt --check > /validation/evidence/release-fmt.log 2>&1
cargo test --offline --lib --no-run --message-format=json > /validation/evidence/release-build.jsonl 2> /validation/evidence/release-build.log
python3 - <<'PY'
import json,pathlib,hashlib
root=pathlib.Path('/validation/evidence');items=[json.loads(x) for x in (root/'release-build.jsonl').read_text().splitlines() if x.startswith('{')]
paths=[a['executable'] for a in items if a.get('reason')=='compiler-artifact' and a.get('profile',{}).get('test') and a.get('executable')];assert len(paths)==1
p=pathlib.Path(paths[0]);(root/'release-executable.json').write_text(json.dumps({'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'size':p.stat().st_size,'mtime_ns':p.stat().st_mtime_ns},indent=2));(root/'release-executable-path').write_text(str(p))
PY
binary=$(cat /validation/evidence/release-executable-path)
"$binary" legacy_api2_ --nocapture > /validation/evidence/release-focused.log 2>&1
cargo test --offline -- --test-threads=2 > /validation/evidence/release-full.log 2>&1
cargo clippy --offline --all-targets > /validation/evidence/release-clippy.log 2>&1
