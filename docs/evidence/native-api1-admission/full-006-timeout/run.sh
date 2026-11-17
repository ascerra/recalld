set -u
mkdir -p /tmp/home
trap 'date -u +%Y-%m-%dT%H:%M:%SZ > /work/output/finished.txt; cat /sys/fs/cgroup/memory.events > /work/output/memory-events.txt; cat /sys/fs/cgroup/memory.peak > /work/output/memory-peak.txt; sha256sum -c /work/output/source.sha256 --quiet > /work/output/source-after.log' EXIT
sha256sum -c /work/output/source.sha256 --quiet || exit 91
date -u +%Y-%m-%dT%H:%M:%SZ > /work/output/started.txt
rustc -Vv > /work/output/rustc.txt
# Same immutable source as005 and same flags/target: no source switch or clean.
timeout --signal=TERM --kill-after=10s 120s cargo fmt --all -- --check > /work/output/fmt.log 2>&1
fmt_result=$?
printf '%s\n' "$fmt_result" > /work/output/fmt.exit
if [ "$fmt_result" -ne 0 ]; then exit "$fmt_result"; fi
timeout --signal=TERM --kill-after=10s 1200s cargo test --all-targets --no-run --locked --offline --message-format=json-render-diagnostics > /work/output/compile.jsonl 2> /work/output/compile.log
compile_result=$?
printf '%s\n' "$compile_result" > /work/output/compile.exit
if [ "$compile_result" -ne 0 ]; then exit "$compile_result"; fi
python3 - <<'INNER'
import pathlib,json,hashlib
out=pathlib.Path('/work/output'); artifacts={}
for line in (out/'compile.jsonl').read_text().splitlines():
 try: item=json.loads(line)
 except ValueError: continue
 if item.get('reason')=='compiler-artifact' and item.get('executable') and item.get('profile',{}).get('test'):
  p=pathlib.Path(item['executable']); artifacts[str(p)]={'name':item['target']['name'],'kind':item['target']['kind'],'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size}
assert artifacts
(out/'executables-before.json').write_text(json.dumps(artifacts,indent=2)+'\n')
INNER
timeout --signal=TERM --kill-after=10s 1800s cargo test --all-targets --locked --offline -- --test-threads=1 > /work/output/tests.log 2>&1
test_result=$?
printf '%s\n' "$test_result" > /work/output/tests.exit
python3 - <<'INNER'
import pathlib,json,hashlib
out=pathlib.Path('/work/output'); artifacts=json.loads((out/'executables-before.json').read_text())
for file,entry in artifacts.items():
 assert hashlib.sha256(pathlib.Path(file).read_bytes()).hexdigest()==entry['sha256'],file
(out/'executables-after-verified.txt').write_text('All compiled test executable hashes unchanged after full run.\n')
INNER
if [ "$test_result" -ne 0 ]; then exit "$test_result"; fi
timeout --signal=TERM --kill-after=10s 600s cargo clippy --all-targets --all-features --locked --offline > /work/output/clippy.log 2>&1
lint_result=$?
printf '%s\n' "$lint_result" > /work/output/clippy.exit
exit "$lint_result"
