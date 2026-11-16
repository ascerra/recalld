set -eu
cd /work
python3 .test-scratch/seal.py verify .test-scratch/tail
cargo fmt --all
python3 .test-scratch/seal.py seal .test-scratch/tail
cargo test --offline --lib original_api -- --nocapture > .test-scratch/tail/focused.log 2>&1
python3 .test-scratch/seal.py binary .test-scratch/tail
cargo fmt --all -- --check > .test-scratch/tail/fmt.log 2>&1
cargo clippy --offline --all-targets > .test-scratch/tail/clippy.log 2>&1
python3 .test-scratch/seal.py binary .test-scratch/tail
python3 - <<'CHECK'
import json,pathlib,hashlib
expected=json.loads(pathlib.Path('.test-scratch/tail/source.json').read_text())
assert all(hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()==h for p,h in expected.items())
pathlib.Path('.test-scratch/tail/result.json').write_text(json.dumps({'focused':0,'fmt':0,'clippy':0,'verifiedInputs':len(expected)},indent=2)+'\n')
CHECK
