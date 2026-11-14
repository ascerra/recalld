# Native profile/observation consumer: preliminary evidence

This is continuing work on `impl/native-insertion-wire`, not a shipping seal,
full-suite result, or public-v2 activation. No standalone helper PR is proposed.
Actual parents are PR122 `abd0371`, PR123 `d7c4d5f`, and observation `cc865c2`.

All runs use Rust1.94.1 in `recalld-ci:1.94`, Docker `--network none --cpus 2
--memory 4g`, two Cargo jobs, immutable read-only source snapshots, and the
assigned private cargo/target. Source roots changed only after preserving the
preceding executed binary hash and cleaning the package in that private target.
No personal data, daemon, home, credentials, or host socket were mounted.

## Completed initial probes

- `dab12d0`: one synthetic typed serializer probe passed (0.52s); no native
  publication claim. Synthetic receipt 4,110,022 bytes; text-plus-structured MCP
  8,483,341 bytes. Repeated IDs and maximum invented hash commitments are not
  valid native state or a historical oversize reachability proof.
- Genuine BEFORE: unchanged PR123 `d7c4d5f` plus one appended test-only producer;
  one maximum-link insertion/reopen test passed (62.83s), 1,024 edges and 1,025
  record proofs, zero embedding-provider calls. Receipt 3,220,502 bytes; existing
  pretty MCP 12,305,907 bytes; compact text-plus-structured MCP 6,648,160 bytes.
  This receipt fits the 8,388,608-byte frame without removing any receipt fields.
  It does not prove that all valid historical receipts fit. Original store bytes,
  receipt bytes, producer instrumentation, logs and source/binary hashes are kept.
- `c9f4654`: three actual profile-ID batch tests passed (3.68s): supplied-vector
  provider-free execution and current-G publication, original fallback identity,
  CAS/key/vector-bit conflict checks, pending reopen using retained Bound defaults
  despite invalid current settings/provider identity, and closed persisted
  mismatch diagnostics with public typed mapping.
- On the same immutable `c9f4654`, two actual observation tests passed (0.24s,
  0.22s) and four contract serialization tests passed. The real failure test covers
  a live caller root without self-drain, unknown FTS/null stamp, accepted0 with
  durable effects still positive, full receipt retention when observation fails,
  always-false observational readiness and unchanged successful barrier shape.

The first observer/contract filters selected zero tests. Those logs are retained
as no-test diagnostics and are not counted as passes; corrected explicit filters
ran the tests above on the identical immutable source. Formatter-copy allowlist
initially stopped at ordering adjacent cfg(test) modules in daemon/server.rs.
That change was inspected and the intended copies completed before testing.

## Preserved transport failure

`855bd6f`: fmt passed; seven focused wire tests finished with five passes and two
failures (60.40s). The profile phase did not execute after that failure.

1. The inherited publication fixture corrupted vector width before reservation.
   With the actual generation-aware owner, reservation publication correctly
   failed before the child ran: HTTP503, notAttempted, no child receipt. The
   correction moves corruption into the provider hook after reservation, so the
   fixture reaches its intended committed-child publication failure.
2. The new post-children/final-barrier fixture reached HTTP207, retained its full
   committed receipt, reported zero accepted tickets and a null FTS stamp. It then
   failed a counter assertion: the inherited provider counted profile lookup,
   dimension lookup and embedding together (3), while the test expected one
   embedding. The correction uses separate profile and embed counters and keeps
   strict receipt/replay/byte/provider/clock assertions.

Both corrections are test-only in `2cb4946`; runtime behavior was not weakened.
Corrected immutable `2cb4946`: fmt passed; the actual post-provider publication
fault passed (1.84s), the final-barrier/observation-failure test passed (4.63s), and
three profile-ID regressions passed (5.31s). The final-barrier test exercised
HTTP207 with null FTS after all children completed and the root finished; its
second case exercised MCP isError with readiness absent and READINESS_UNKNOWN.
Both retained complete receipts and repaired on same-key replay after reopen,
with exact retained receipt identity (only replayed=true on replay), unchanged
canonical bytes, and no extra embed/profile calls or clock samples. No full
suite or clippy run on this composition is claimed.

Final single
wire/budget work, historical-selector fixtures, all required transport regressions,
full shipping-tree validation, evidence seal and publication remain outstanding.
