# Query-owned exposure inputs

Minimal parent: actual #96 `387c7118864040c68d7d28c44fb2600ade520a45`.
This prerequisite contains no incremental publication, graph or producer ancestry.

`QueryEngine::record_exposures_sampled` now passes its configured `DecayConfig`
and multiplier to `AccessRecorder::record_access_batch_for_publication`, together
with the original scope, accepted work and optional already sampled clock. The
additive trait default delegates to the existing sampled/non-sampled methods.
`AccessObservation::from_receipts` preserves the receipts and the actual existing
full derived completion path. It is a Rust-only result, not a wire/readiness proof.

The actual QueryEngine regression uses the production legacy storage recorder
through that default. It checks nondefault policy forwarding, no second sampled
clock, one legacy recorder clock sample when absent, canonical timestamps,
distinct returned receipts, completed view stamps, empty-batch byte invariance,
and drained owned work. No native exposure writer or serving activation is added.
Pascal owns that later adapter; it can return `from_receipts` and retain full
completion without depending on the unfinished incremental candidate.

Validation source SHA256:
`d03c675df6462b45c038f50e12c0514d2865186786951b161bdfef35bbde9083`.
Validation passed: 1 focused regression + 1001 library + 2 CLI tests; fmt passed,
clippy exited 0 with warnings. Exact source/binary/log hashes and environment are
in [the evidence](../evidence/access-policy-forwarding/REPORT.md).
