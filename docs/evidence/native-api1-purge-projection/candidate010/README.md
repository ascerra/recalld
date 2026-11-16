# Candidate010: exact bounded acceptance GREEN

Exact test source8c990c010355243863e7f2bd4865e516b73efa63, runtime unchanged369d8e92.
22focusedPASS/0FAIL40.64s; fmt passed. Candidate009's poisoned-lock test observation
is corrected by capturing the same tracker before injection; all three actual
bounded unknown modes now pass, including exact child ID, full known start,
no invented receipt/readiness, unresolved work and close/reopen reconciliation.

Actual outer E=3868B. Exact Value receipt N/start861/child9452/final377B;
maximum observed typed-error frames4663/13809/4136B respectively fit E+2N
5590/22772/4622B. Unknown with no receipt is236B. These are measured fixtures,
not a universal receipt maximum. Every tested N uses the actual Value projection;
the inverse budget returns N at E+2N and N-1 one byte below. Tests cover NUL,
quote/backslash and UTF8 diagnostics, I/O/control/recovery errors, nested committed
receipt, allocated-ID commit unknown, worker unknown with/without receipt and
MetricsUnavailable only with the actual validated final matching receipt.

Both unbounded and bounded cancellation retain worker publication/graph guards
through caller loss and preserve unresolved tracking. Actual400-peer/32history
and1000-row negotiated frames remain1,302,161B and1,572,891B; old1MiB refuses
before writes/IDs. Historical oversized replay/name reuse, profile caps and
competing continuation remain green. The following test-only tail adds numeric
policy, batch reservation and physical-compaction attempts to the existing paused
writer-exclusion race; it does not change this run's source or runtime.

OfflineRust1.94.1/2CPU/4GiB, one reused package-cleaned target. memory.max0/OOM0.
No full-suite/publication claim. Raw /tmp/recalld-native-api1-purge-validation/candidate010.
