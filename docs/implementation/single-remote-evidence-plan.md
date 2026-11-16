# Malformed Remote API1 single reply evidence

Exact isolated base: `b694d4f2f6fa067eaeef88b360db527060b05f9e`.
Halley explicitly released `serialization/store_api1.rs`,
`daemon/store_capability_tests.rs`, and strictly necessary projection consumers
to Huygens alone. Active PR181 validation and its worktree remain immutable.
Feynman reviews the receipt contract; he owns no edits/builds for this child.
The coordinator approved this bounded work under the user's standing authorization.

Source finding, not yet an executed regression: malformed capable Remote single
wrappers extract a typed receipt and pass it through `native_single::failure`.
Its shared batch mapper relocates `BatchReserved` to `reservation_receipt`, which
the single mapper does not copy. A malformed wrapper also cannot authenticate a
receipt as evidence that the current single operation committed.

First reproduce through the existing negotiated same-connection Unix transport,
using an actual legacy batch reservation retained across close/reopen. This is
genuine parent evidence, not a claim a valid native single producer returns a
batch reservation. Preserve exact store bytes, full receipt and request identity;
assert the next request is ping, never a second mutation/fallback.

Minimal proposed correction: API1-only uncertain evidence carrier and projection,
following the existing batch `receivedEvidence` convention. A complete decoded
receipt remains unverified received evidence, separate from the current write's
receipt and operation ID. Current commit state remains unknown, no readiness or
success data is asserted. No change to native_single's valid known-commit mapper,
batch reservation mapping, public2 activation, storage codec or durable state.

Validate the actual RED before runtime changes, then the same regression and
affected single/daemon/API1 compatibility tests, fmt and clippy on frozen source.
Review exact child with Feynman and hand it to Halley; final integrated composition
gates remain separate. Use one private cargo/target, isolated offline Docker with
2 CPUs/4 GiB initially. Failed raw evidence remains protected; cleanup is centrally
coordinated after results/integration clearance.
