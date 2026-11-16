set -eu
cd /source
cargo test --offline --all-features --no-run --message-format=json > /validation/evidence/corrected-build.jsonl 2> /validation/evidence/corrected-build.log
python3 - <<'PYINNER'
import pathlib,json,hashlib
p=pathlib.Path('/validation/evidence');a=[json.loads(x) for x in (p/'corrected-build.jsonl').read_text().splitlines() if x.startswith('{')];b=[x['executable'] for x in a if x.get('reason')=='compiler-artifact' and x.get('profile',{}).get('test') and x.get('executable')];out=[]
for name in b:
 f=pathlib.Path(name);out.append({'path':str(f),'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'size':f.stat().st_size,'mtime_ns':f.stat().st_mtime_ns})
assert len(b)>=2,b
(p/'corrected-executables.json').write_text(json.dumps(out,indent=2))
PYINNER
cargo test --offline --all-features api1_public_ -- --nocapture > /validation/evidence/corrected-focused.log 2>&1
cargo test --offline --all-features scope_assertions -- --nocapture > /validation/evidence/corrected-scope.log 2>&1
cargo test --offline --all-features > /validation/evidence/corrected-full.log 2>&1
cargo fmt --all -- --check > /validation/evidence/corrected-fmt.log 2>&1
cargo clippy --offline --all-targets --all-features > /validation/evidence/corrected-clippy.log 2>&1
