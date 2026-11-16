#!/bin/sh
cd /work
status=0
cargo clean -p recalld > /evidence/clean.log 2>&1
for filter in prepared_evidence_ strict_query_ similar_explanation_ coherent_rif_explanations; do
 cargo test --locked --offline --all-features --lib "$filter" -- --nocapture > "/evidence/focus-$filter.log" 2>&1
 result=$?
 printf '%s %s\n' "$filter" "$result" >> /evidence/status.txt
 if [ "$result" -ne 0 ]; then status=$result; break; fi
done
if [ "$status" -eq 0 ]; then
 python3 tests/query_schema_conformance.py /evidence/schema-fixtures > /evidence/schema.log 2>&1
 result=$?; printf 'schema %s\n' "$result" >> /evidence/status.txt
 if [ "$result" -ne 0 ]; then status=$result; fi
fi
printf '%s\n' "$status" > /evidence/exit-code
cat /sys/fs/cgroup/memory.events > /evidence/memory.events
cat /sys/fs/cgroup/memory.peak > /evidence/memory.peak
exit "$status"
