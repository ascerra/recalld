#!/bin/sh
date -u +%Y-%m-%dT%H:%M:%SZ > /evidence/focus.started
/target/debug/deps/recalld-8b31964f64b2cfc4 api1_point_ --nocapture > /evidence/focus.log 2>&1
status=$?
echo "$status" > /evidence/focus.exit
date -u +%Y-%m-%dT%H:%M:%SZ > /evidence/focus.finished
cat /sys/fs/cgroup/memory.events > /evidence/focus-memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/focus-memory.peak
exit "$status"
