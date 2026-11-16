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

## Published121 parent join

`e11eb12` joins published PR121 `3fa91d4` directly for retained namespace binding.
Curie's `866482da` source unions were used as a conflict-resolution reference,
not as a shipping parent or inherited validation. Existing canonical-vector and
exact-scan additions, batch CAS/profile preparation, observation, and the actual
native-purge pending-reader checks remain. This first immutable run stopped at
two rustfmt module-order differences; no tests ran and the failed log is retained.

The ordering-only correction is frozen in `f7b6d625c3c44e68683d67a9d643e014c3469b20`.
Formatting and nine focused tests passed: actual publication failure (1), final
barrier failure and same-key repair (1), profile-ID batch cases (3), actual
failure-readiness observation (1), and retained namespace-binding cases (3).
The binding cases cover retired/recreated names, foreign guards/missing identity,
and retained unknown profile provenance. This is existing API integration proof,
not proof of an implemented single-write wire consumer.

Both source snapshots' 307 file hashes match pre/post; the executed binary hash,
commands, results and durable-copy manifest are in `join-f7b6d62.json`. The private
package target was cleaned before the changed source build. Tests used offline
Docker with two CPUs, 4 GiB and Rust1.94.1; both completed test containers were
removed. All 34 evidence files, including the stopped formatter run, were copied
and SHA-verified in the referenced work-notes artifact directory. No full suite
or clippy run on this join is claimed, and this active branch is not READY for
integration or publication.

## Single-wire validation after disk cleanup

Runtime `d12a9a3` used a new bounded private target, the protected offline Cargo
cache, and an immutable source snapshot. Formatting passed; focused execution
finished with20 passed,1 failed,1 explicitly ignored in19.67s. The ignored dense
historical replay requires its separate read-only BEFORE artifact mount. The
full suite and that separate replay did not run after the failure.

The new retired-namespace replay returned the original complete receipt and
ready=true, then failed its strict whole-store byte equality assertion. Only
`meta.db` differed. Fixture setup had invoked raw core purge and namespace
creation without the real derived completion owner. The correction will assert
that setup is unpublished, complete it through the real owner, assert unchanged
canonical generation and current derived publications, then capture the byte
baseline. Replay must still preserve all bytes, clock/provider counts and the
original namespace ID after same-name recreation. No runtime relaxation is made.
The original failed source, log and executed binary SHA
`aef40c5b89d4d415179a5df5d0a879c2c1cc3b5b4a325ca4ed5d037877f101a0`
remain in `run-d12a9a3` under the active validation root until results/publication
cleanup handoff. Current consumer is not READY from this run.

The corrected `cb81448` passed21 focused tests (one explicit ignored fixture) in
20.45s, including setup-stale/current-publication, unchanged generation and strict
retired replay byte identity. The separate genuine BEFORE test then failed at
its framed RPC assertion: the helper had skipped `ping` negotiation, so the
server correctly refused a3,221,546-byte reply at the legacy1,048,576-byte limit.
Exact HTTP receipt and MCP text/structured equality had passed before that
assertion. The real daemon client negotiates protocol2's8MiB limit; the correction
adds that existing handshake to the test-only helper, not production code or
transport limits. Full-suite/clippy did not run on `cb81448`. Executed binary SHA:
`523fa6e66d706dfc675a2d380cd34b31bf4cb907355c81fadb8d86effce9e29d`.
