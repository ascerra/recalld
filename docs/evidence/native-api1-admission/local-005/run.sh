set -u
mkdir -p /tmp/home
trap 'date -u +%Y-%m-%dT%H:%M:%SZ > /work/output/finished.txt; cat /sys/fs/cgroup/memory.events > /work/output/memory-events.txt; cat /sys/fs/cgroup/memory.peak > /work/output/memory-peak.txt; sha256sum -c /work/output/source.sha256 --quiet > /work/output/source-after.log' EXIT
sha256sum -c /work/output/source.sha256 --quiet || exit 91
date -u +%Y-%m-%dT%H:%M:%SZ > /work/output/started.txt
rustc -Vv > /work/output/rustc.txt
cargo clean -p recalld > /work/output/clean.log 2>&1 || exit 92
timeout --signal=TERM --kill-after=10s 120s cargo fmt --all -- --check > /work/output/fmt.log 2>&1
printf '%s\n' "$?" > /work/output/fmt.exit
timeout --signal=TERM --kill-after=10s 1200s cargo test --lib --no-run --locked --offline --message-format=json-render-diagnostics > /work/output/compile.jsonl 2> /work/output/compile.log
compile_result=$?
printf '%s\n' "$compile_result" > /work/output/compile.exit
if [ "$compile_result" -ne 0 ]; then exit "$compile_result"; fi
python3 - <<'INNER'
import json,pathlib,hashlib,subprocess
out=pathlib.Path('/work/output')
bins=[]
for line in (out/'compile.jsonl').read_text().splitlines():
 try: item=json.loads(line)
 except ValueError: continue
 if item.get('reason')=='compiler-artifact' and item.get('executable') and item.get('profile',{}).get('test') and 'lib' in item.get('target',{}).get('kind',[]): bins.append(item['executable'])
assert len(bins)==1,bins
binary=pathlib.Path(bins[0]); (out/'executable.sha256').write_text(hashlib.sha256(binary.read_bytes()).hexdigest()+'  '+str(binary)+'\n')
cmd=['timeout','--signal=TERM','--kill-after=10s','900s',str(binary),'native_catalog','--nocapture','--test-threads=1']
(out/'test-command.json').write_text(json.dumps(cmd)+'\n')
with (out/'tests.log').open('w') as log: code=subprocess.call(cmd,stdout=log,stderr=subprocess.STDOUT)
(out/'tests.exit').write_text(str(code)+'\n')
raise SystemExit(code)
INNER
exit $?
