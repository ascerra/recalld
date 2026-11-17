set -u
mkdir -p /tmp/home
trap 'date -u +%Y-%m-%dT%H:%M:%SZ > /work/output/finished.txt; cat /sys/fs/cgroup/memory.events > /work/output/memory-events.txt; cat /sys/fs/cgroup/memory.peak > /work/output/memory-peak.txt; sha256sum -c /work/output/source.sha256 --quiet > /work/output/source-after.log' EXIT
sha256sum -c /work/output/source.sha256 --quiet || exit 91
date -u +%Y-%m-%dT%H:%M:%SZ > /work/output/started.txt
rustc -Vv > /work/output/rustc.txt
python3 /work/continue.py
result=$?
printf '%s\n' "$result" > /work/output/tests.exit
if [ "$result" -ne 0 ]; then exit "$result"; fi
timeout --signal=TERM --kill-after=10s 600s cargo clippy --all-targets --all-features --locked --offline > /work/output/clippy.log 2>&1
result=$?
printf '%s\n' "$result" > /work/output/clippy.exit
exit "$result"
