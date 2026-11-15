set -eu
cd /source
cargo clean --offline -p recalld > /validation/package-clean.log 2>&1
cargo test --offline --all-features --all-targets --no-run --message-format=json > /validation/build.jsonl 2> /validation/build.log
python3 - <<'ARTIFACTS'
from pathlib import Path
import hashlib,json
p=Path('/validation')
items=[json.loads(l) for l in (p/'build.jsonl').read_text().splitlines() if l.startswith('{')]
names=sorted({e['executable'] for e in items if e.get('reason')=='compiler-artifact' and e.get('executable')});assert names
(p/'executables.json').write_text(json.dumps([{'path':f,'sha256':hashlib.sha256(Path(f).read_bytes()).hexdigest(),'bytes':Path(f).stat().st_size,'mtime_ns':Path(f).stat().st_mtime_ns} for f in names],indent=2)+'\n')
ARTIFACTS
status=0
for filter in query_feedback_chronology_ strict_query_ api1_public_ adopted_public_ inherited_policy_ legacy_lifecycle decoder_native_ native_record_context_; do
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
p=Path('/validation');counts={}
for name,minimum in [('query_feedback_chronology_',4),('strict_query_',9),('api1_public_',18),('adopted_public_',4),('inherited_policy_',11),('legacy_lifecycle',21),('decoder_native_',3),('native_record_context_',2)]:
 s=(p/('focus-'+name+'.log')).read_text();passed=sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',s)));assert passed>=minimum,(name,passed,minimum);assert 'test result: FAILED' not in s;counts[name]=passed
(p/'nonzero-focus-gate.json').write_text(json.dumps(counts,indent=2)+'\n')
GATE
cargo test --offline --all-features --all-targets > /validation/full.log 2>&1
cargo fmt --all -- --check > /validation/fmt.log 2>&1
cargo clippy --offline --all-targets --all-features > /validation/clippy.log 2>&1
