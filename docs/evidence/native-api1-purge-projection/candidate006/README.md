# Candidate006: actual legacy frame failure

Frozen test-only source 4853c6e5 adds an actual 1000-row CHUNK_LIMIT fixture and
numeric malformed-input controls; production remains c8961d23. Offline Rust1.94.1,
2CPU/4GiB, existing package-cleaned private target. Formatting passed. Eleven
focused tests passed; one failed in 33.62 seconds. The expanded typed overflow and
raw non-finite/parser cases passed. No full-suite result is claimed.

Actual legacy insertion, schema migration, owned native purge and daemon framing
reached canonical Complete. The matched child receipt is 1,464,530 bytes; its escaped
daemon response is 1,572,891 bytes, exceeding the unnegotiated 1,048,576-byte limit.
The real response fallback returned -32005 with generic get_memory advice instead
of -32006 with the full known receipt. This is a reachable postcommit evidence-loss
failure, not a syntax-only example or timeout. This fixture bounds row count only;
it does not establish a maximum including all edge/history payloads.

No receipt truncation, transport change or start-receipt substitution has been
implemented. Coordinator and wire owner received the exact failure for a bounded
source-plan decision. The failed source, executable and complete log remain in the
protected raw root identified by manifest.json. Memory reached 4GiB with 356 max
reclaim events; oom/oom_kill were zero. All owned test containers retired normally.
