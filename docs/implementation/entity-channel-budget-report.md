# V2 total entity-channel admission

Minimal published parent: PR140 `5f0e4f02d68ff3b3ed56a899227e24cfdaf8f92f` (already includes PR133 and PR132). Inventory commit `9d3dbf6` precedes runtime `49b1ca3`. No new storage, provider, native admission, wire, RIF or FTS parent is required.

## Problem and correction

At `L=2`, an overlapping vector/FTS candidate union caused the entity reader to return eight members, although the v2 channel allowance is four. The old code requested `2L + union.len()` and capped only novel entity-only candidates. Channel provenance consequently also described entity evidence outside the v2 allowance.

V2 now requests the first `2L` canonical-eligible matches from the existing namespace-partitioned, shared-count/UUID-ordered entity reader. Overlapping direct IDs consume that entity allowance before the union; a direct ID outside it remains a direct candidate without entity-channel provenance. Vector/FTS union membership is not removed or capped by this correction. Existing fusion, entity overlap/recall weights, strength, temporal boost, version resolution and final ordering are unchanged. Entity overlap remains a baseline score feature even for direct IDs that were not admitted by the entity-recall channel; channel membership is not an additive score term.

V1 explicitly retains its novel-only entity allowance. This is a versioned `retrieval-v2-default` budget correction, distinct from the shared canonical eligibility fixes. Both versions still reject foreign, filtered, ghost and tombstone rows before the entity allowance. The validated user limit is at most 200, so the v2 entity allowance is at most 400; this is an admission bound, not an examination/allocation/performance bound.

`offered` and `admitted` count actual channel members before union deduplication and later score/version/result truncation. Examination, total eligibility, exhaustion and candidate/work limiting remain `None` because this reader interface does not measure them. A full page is not evidence of exhaustion or of unseen matches. Public consumers must preserve unknowns; source coverage measurement remains separate A2 work.

## Actual regression

`entity_total_budget_counts_vector_fts_overlap_before_union` uses the real flat vector index, namespace entity index, SQLite FTS adapter and QueryEngine. It covers zero/partial/full overlap, current/history, reversed record insertion, 100 higher-overlap foreign/filtered/ghost/tombstone records, a direct-only high-scoring ID outside the entity allowance, returned provenance, and the named v1 candidate-pool behavior. No transport or ordinary native activation is implied by this engine fixture.

The unchanged production parent with the new regression failed: overlap=2, entity `offered=Some(8)` versus expected `Some(4)`. It ran one test (0 passed, 1 failed, 1,041 filtered), exit 101. Frozen source manifest SHA-256 `a1f1e243fd7a8b76a7bf49da6125b78932782a416851ed6070c9cfa0bcda7203`; actual library test executable `8598afa4cd1ee24aa43d64b3175404ffcc9cdba09dbd04aae589911b7e15fd0b`; full log `9e3b046f617780053fe0f12cafe33745cb14d3d7b97b647961178e7b76b2498b`. Both mounted snapshot and worktree matched the manifest after the run. Failure evidence is retained; the runtime fix was applied only afterward.

Corrected runtime `49b1ca3dc9c6e5fcf626b1ca9a12be738c86d6e0` passed **3 focused checks, 1,042 library tests and 2 CLI tests**, plus fmt and clippy (exit 0). The full library suite took 421.69 seconds; this is validation duration, not a performance measurement. The new regression is one test covering 12 v2 overlap/order/mode combinations and six named v1 comparisons; the other two focused checks are existing regressions. No doctest claim follows from `--all-targets`.

Clippy reported 707 library warnings, 776 library-test warnings (685 duplicates), and one binary warning (repeated for the binary test target), matching the parent warning summaries. No warning cleanup is included.

All 233 allowlisted source files matched the immutable mounted snapshot, worktree and committed runtime after validation. GREEN source manifest SHA-256 `52866775c321af6d56ae986d705e72d7bccfed06d0425c8d369401157553426a`; library test executable `25528c36f9c07912bf6af24e77ba5cda1eff63635a338876b69c5a0ff0cd5a4a`; full log `e85b7d609d4f50cb8b51e67f1f058788b6dde269eb01c24612c8a1451659d586`; verification manifest `ade9d0a70c4b43d21ad070fde884b30336b3fd900174d4e9c21e32c79d7c8ef5`. Actual CLI/binary hashes and individual result lines are in [GREEN verification](evidence/entity-channel-budget/green/verification.json); [RED verification](evidence/entity-channel-budget/red/verification.json) retains the original failure.

The corrected container ran from 2026-11-15T00:17:57Z to 00:27:24Z, exited 0 without OOM, and was removed after evidence sealing. The earlier RED container was also removed. Runtime/source/protocol were not changed during either run.

## Validation boundaries and retention

Docker `recalld-ci:1.94`, image `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`, Rust 1.94.1, network none, two CPUs, 4 GiB, two Cargo jobs. Only allowlisted frozen source (read-only), a validation script, one private target and the retained private Cargo cache are mounted. No home, credentials, daemon/data or Docker socket mount. Before the source switch, the RED executable hash was preserved; `cargo clean -p recalld --offline` then removed only this package from the owned target. Dependencies were retained.

Small source/verification/script evidence belongs in this repository. Raw runs remain disposable after publication and recorded results, through the central cleanup owner. No new HOME raw archive is required. Canonical scanning and uncapped entity-posting enumeration costs remain; no latency, quality, full-A2 completion or default/public-v2 promotion claim is made.
