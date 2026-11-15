# Legacy-disk lifecycle consumer validation

Actual parents: PR136 `047ebe086b01747c0dec38228141a3696d885d29` plus PR132
`5864c09050e532454c97ba8e73e00e7ba1670fb4`, real join `fd73618`.
Implementation baseline `6e225e3`; full-suite runtime/test-source
`ebb3b251bb6c6a4dea09752386ab291b4e30b13d`; final reviewed decoder runtime
`5a0c821b2f99f82564a76b5fe816255585480f2b`. No manual/compound/public-routing parent.

## Scope

The existing owned engine mode selects the physical writer independently of the
original application contract1|2. Prepared MCP and HTTP lifecycle consumers and
the new private daemon aliases reach actual legacy metadata transactions and
canonical publication. Existing native-only aliases still reject legacy stores
before writes. New aliases are nonretrying, reject unknown old handlers, and
never fall back. Native replies omit `legacyData` and preserve their old shape.
Public2 routing and ordinary native serving remain gated.

Feedback resolves the configured policy through the shared planner, checks
matching replay/CAS before its one injected clock sample, and atomically writes
state/history/receipt. Forget uses the existing tombstone/effects transaction;
it has no timestamp field requiring a fabricated clock sample. Unasserted old1
request hashes are unchanged; supplied names become checked/hash-bound assertions.
PR132 supplies the real original1|2 exposure recorder. This child proves actual
legacy2 query observation and false-mode nonobservation, not keyed search replay.

Tests exercise prepared MCP text/structured output, prepared HTTP, real daemon
framing, replay before changed policy/NeverClock/staleCAS, cross-origin key
conflicts, independent unkeyed events, strict reopen, and real purge/name reuse.
Retained terminal identity resolves scope before fresh CAS even with an explicit
assertion. No default namespace substitutes for a by-ID scope. Genuine old1
writer backup/receipts replay unchanged; no historical numerical state is guessed.

Known commits retain their full receipt on publication/binding/decoder failures.
Actual uncertain tombstone commit preserves its known operation ID without
inventing a receipt; reopen reconciles it. A lost worker reports unknown without
inventing an ID. Failures never self-drain their own accepted root. Decoder
identity/version validation rejects unrelated or future receipts; matching known
receipts survive malformed views. This is validation, not receipt authentication.

## Before/after evidence

1. The actual pre-correction binary failed three tests: prepared legacy2 feedback
   and forget hit native-only admission; metadata tombstone rejected contract2.
   Its fixture-generator run produced real API1 exposure/feedback/forget receipts.
   The filter also counted the generator's no-op as a pass; it is now ignored.
2. First correction passed11/failed1 (including that generator no-op): explicit
   reused namespace reached stale CAS instead of terminal scope rejection. The
   correction performs retained-identity resolution even with explicit scope.
3. Corrected scope run passed15 (14 actual tests plus generator no-op). Frozen
   baseline6e225e3 then passed1186 library/2CLI,15focused/1ignored,fmt/clippy0
   with909 library/973 test warnings (882duplicate).
4. A new synthetic malformed-peer test used a real receipt but substituted its
   target. Baseline decoder returned Committed instead of Unknown:0pass/1fail.
   The subsequent fake-server Closed panic followed that assertion failure; it
   is not a separate daemon defect. The fix reuses the existing matching-receipt
   predicate for both ordinary and malformed replies.
5. Full-suite baseline `ebb3b25` passed **1188 library +2 CLI**, **17 focused**, fmt,
   and clippy exit0 with909 library/973 test warnings (882duplicate). One fixture
   generator and four doctests are ignored. All287 recorded inputs were verified
   unchanged. Full518.79s is test duration, not a performance comparison.
   Executable SHA256: `e15f13170c537571f6d900b63c7f8e1d3a52c456ebaad55adfbe316ec343979b`.

The genuine fixture ZIP SHA256 is
`448d8e7d3cf963bb09001e1e5269d426a4a8741c595a70c81e1c0828c2291c02`.
It is before-correction writer output, not handcrafted rows. Old-handler tests
emulate historical rejection with actual current client/framing; they do not run
a historical daemon binary. Native old2 fixture coverage is inherited unchanged.

## Isolation and limits

All executions use `recalld-ci:1.94`, Rust1.94.1, networknone,2CPU/4GiB,jobs2;
read-only source and read-only shared dependency registry plus one owned private
Cargo/target. No HOME, credentials, Docker socket or personal stores are mounted.
Source hashes, binary SHA/size/mtime, focused raw results and full result excerpts
are recorded. Test durations are not performance evidence. Clippy is not warning-free.

This does not activate public2, change API1 outward response schemas, authenticate
arbitrarily rewritten stores, provide full deterministic IDs, or complete A3.
Huygens owns public routing/shared readiness envelopes; Curie owns insertion/graph
and compound context; Feynman owns catalog/purge/compaction history. A3 acceptance
inventory follows publication separately. Cleanup receives the whole completed
run root after recorded results/publication; tracked source/reports remain protected.
No new raw HOME archive is required under the user's retention override.

## Final review correction and exact validation attribution

Main review found native receipt matching omitted feedback quality and requested
namespace, and allowed an error claiming Committed without a receipt. Actual
native-writer receipts reproduce both identity gaps in success, error and
malformed replies (0pass/2fail); a separate actual-receipt fixture reproduces the
receipt-free Committed claim (0pass/1fail). An initial test-only compile failure
used a UUID for the existing u32 namespace ID; its diagnostic/source are retained,
no executable was produced, and it is not reported as a runtime failure.

Final shared matching requires native feedback quality and any asserted namespace
against the typed transition; native forget uses that same namespace assertion.
Error Committed is equivalent to a present matching receipt with matching
operation ID. Legitimate Unknown can still carry an allocated operation ID with
no receipt; NotCommitted can have neither. Matching known receipts survive
malformed replies as committed failures. No request hash is reconstructed from
unavailable context, and no receipt/operation ID is invented.

The final source passed3 decoder probes (included in17 native lifecycle protocol
tests), plus17 separate legacy tests/1ignored fixture generator, fmt and clippy0
WITH909 library/973 test warnings (882duplicates). All287 inputs remained unchanged.
The full1188+2 run belongs to ebb3b25 BEFORE these reviewed predicates; it is not
claimed as a full run of the final runtime. Per review instruction the bounded
runtime tail receives focused validation plus exact published-head CI; no local
full repeat is attributed here. See validation.json for both executable hashes.
