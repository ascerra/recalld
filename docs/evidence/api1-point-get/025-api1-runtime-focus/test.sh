#!/bin/sh
date -u +%Y-%m-%dT%H:%M:%SZ > /evidence/test.started
/target/debug/deps/recalld-8b31964f64b2cfc4 api1_point_before --nocapture --test-threads=2 > /evidence/focus.log 2>&1
status=$?
echo "$status" > /evidence/focus.exit
date -u +%Y-%m-%dT%H:%M:%SZ > /evidence/test.finished
cat /sys/fs/cgroup/memory.events > /evidence/test-memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/test-memory.peak
exit "$status"
