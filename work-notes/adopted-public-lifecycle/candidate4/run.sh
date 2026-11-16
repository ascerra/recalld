set -eu
cd /source
cargo test --offline --all-features --no-run --message-format=json > /validation/build.jsonl 2> /validation/build.log
python3 - <<'PY'
from pathlib import Path
import json,hashlib
p=Path('/validation');items=[json.loads(l) for l in (p/'build.jsonl').read_text().splitlines() if l.startswith('{')];names=sorted({e['executable'] for e in items if e.get('reason')=='compiler-artifact' and e.get('executable')});assert names
(p/'executables.json').write_text(json.dumps([{'path':f,'sha256':hashlib.sha256(Path(f).read_bytes()).hexdigest(),'size':Path(f).stat().st_size,'mtime_ns':Path(f).stat().st_mtime_ns} for f in names],indent=2)+'\n')
PY
status=0
for filter in adopted_public_ api1_public_ inherited_policy_ legacy_lifecycle decoder_native_; do
 if cargo test --offline --all-features "$filter" -- --nocapture > "/validation/focus-$filter.log" 2>&1; then
  echo "$filter exit0" >> /validation/status.log
 else
  result=$?; echo "$filter exit$result" >> /validation/status.log; status=1
 fi
done
if test "$status" != 0; then exit 1; fi
python3 - <<'GATE'
from pathlib import Path
import re,json
counts={}
for name,expected in [('adopted_public_',4),('api1_public_',18),('inherited_policy_',11),('legacy_lifecycle',21),('decoder_native_',3)]:
 s=(Path('/validation')/('focus-'+name+'.log')).read_text()
 results=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed;',s)
 passed=sum(int(p) for p,f in results);assert passed>=expected,(name,passed,expected);assert not re.search(r'test result: FAILED',s)
 counts[name]=passed
Path('/validation/nonzero-focus-gate.json').write_text(json.dumps(counts,indent=2)+'\n')
GATE
cargo test --offline --all-features > /validation/full.log 2>&1
cargo fmt --all -- --check > /validation/fmt.log 2>&1
cargo clippy --offline --all-targets --all-features > /validation/clippy.log 2>&1
