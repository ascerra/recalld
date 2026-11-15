# Native forget accepted coordinator

Runtime `51501b6` uses only actual PR96 `387c7118864040c68d7d28c44fb2600ade520a45`.
No maintenance/sweep/purge/graph/catalog parent is imported. The private accepted
entry calls the existing native tombstone transaction, retains the native request
hash, checks replay before capture/current policy/clock, and awaits actual derived
completion with the full receipt on known failure. Caller-configured policy and
clock are passed through; no second runtime source is created. Worker-owned
publication/graph guards survive waiter cancellation. Ordinary schema2 stays gated;
Huygens owns the public MCP/HTTP/RPC wire and envelope integration.

Five actual focused checks pass:

- Concurrent same-key operations admitted before closure finish through existing
  child tickets, commit once, sample time once, preserve anchors/history, clamp a
  regressed phase marker, publish all current-generation views and strictly reopen.
- Actual vector-index dimension failure preserves the full committed receipt;
  replay with invalid current policy/clock and old CAS repairs publication without
  another lifecycle write or clock call. A name assertion failure retains the
  known receipt without changing source bytes.
- Contract1, wrong stable scope, wrong explicit name and stale CAS fail before
  time or bytes change; namespace mismatch retains its typed error.
- A fresh key on an existing tombstone commits a typed durable receipt and leaves
  the record unchanged. Matching replay retains that operation; changing its ID
  conflicts before policy/clock, preserving the other live record.
- Cancellation while the actual mutation worker waits for storage retains both
  publication and graph guards, leaves unresolved work and rejects readiness.

The first focused run was3PASS/1FAIL: its phase-marker assertion expected old3000
instead of max(oldmarker,effective5000). Runtime matched the existing monotonic
rule. Exact source, binary SHA/mtime and raw failure are preserved. Correcting only
that assertion produced4PASS; adding the terminal-key contract produced5PASS.
These are not hidden runtime retries or a newly claimed lifecycle defect.

Retired names require an actual validated purge-witness consumer before historical
name assertions can succeed. Until then the known receipt is retained with a
fail-closed error, never rebound via a reused name. No new receipt/schema codec,
planner, public serving activation, broad ReadView, deterministic-ID or performance
claim is introduced. Native feedback accepted coordination is a subsequent slice.

## Final validation

Runtime `51501b6aef264f7e9b06744b29fba5df8c67f7ca`:1005library+2CLI+5focused
PASS;4doctestsignored;fmtPASS;clippyexit0 WITH warnings:

- warning: `recalld` (lib) generated 697 warnings (run `cargo clippy --fix --lib -p recalld` to apply 83 suggestions)
- warning: `recalld` (bin "recalld" test) generated 1 warning
- warning: `recalld` (bin "recalld") generated 1 warning (1 duplicate)
- warning: `recalld` (lib test) generated 764 warnings (676 duplicates) (run `cargo clippy --fix --lib -p recalld --tests` to apply 6 suggestions)

Source manifest225inputs `2187f661d6a09766ade0bc5e8f1d4f7328e36cf8142e5cdfe6754393a8aab58e` verifiedbefore/after.
All tests used synthetic offline Docker Rust1.94.1,2CPU/4GiB,2jobs,privatecargo/target,
nohome/credentials/socketmounts. Ownpackageclean aftercache/worktreeswitch recorded.
