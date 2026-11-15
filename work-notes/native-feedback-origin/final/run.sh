set -eu
cd /work
python3 .test-scratch/seal.py verify .test-scratch/final
cargo fmt --all
python3 .test-scratch/seal.py seal .test-scratch/final
cargo test --offline --lib original_api -- --nocapture > .test-scratch/final/focused.log 2>&1
cargo test --offline --lib native_unsupported_contracts -- --nocapture >> .test-scratch/final/focused.log 2>&1
cargo test --offline --lib native_feedback_rejects_quality_contract_scope_name_and_cas_without_time_or_writes -- --nocapture >> .test-scratch/final/focused.log 2>&1
python3 .test-scratch/seal.py binary .test-scratch/final
cargo test --offline > .test-scratch/final/full.log 2>&1
cargo fmt --all -- --check > .test-scratch/final/fmt.log 2>&1
cargo clippy --offline --all-targets > .test-scratch/final/clippy.log 2>&1
python3 .test-scratch/seal.py binary .test-scratch/final
python3 - <<'PY'
import json,pathlib,hashlib
expected=json.loads(pathlib.Path('.test-scratch/final/source.json').read_text())
assert all(hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()==h for p,h in expected.items())
pathlib.Path('.test-scratch/final/result.json').write_text(json.dumps({'focused':0,'full':0,'fmt':0,'clippy':0,'verifiedInputs':len(expected)},indent=2)+'\n')
PY
