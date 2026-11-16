# Public sweep clock boundary — validation report

Status: local validation complete; sealed for publication review. No PR or READY handoff for this branch yet.

Sole parent PR147 `50da71abee66cea159988a203d286f570f091718`.
Production correction `4c47c5aabd2bf900916de6d8e82c3b41617efa09`;
final validated source `07681e18b2ef74aa2c9c38d13d8052a631194839`.
The later two commits change only the test snapshot helper.

## Compatibility decision and implementation

This deliberately restricts API1's previously advertised historical sweep time.
It does not characterize that prior advertised behavior as an exploit or an
unintended historical implementation bug. A3 §3 requires production writes to
use the configured clock; an internal read-only as_of cannot authorize writes.

POST /decay/sweep rejects presence of either asOf or as_of before calling the
adapter, with HTTP400/BAD_REQUEST, field asOf and message:
`Public sweeps use the configured clock; remove asOf/as_of`.
Presence includes null, wrong types and repeated keys. Omitted time and unrelated
unknown fields retain API1 behavior. Malformed JSON is rejected by the existing
Axum extractor; its existing plain-text errors are not converted to new envelopes.

Only api/models.rs and api/handlers.rs change production behavior. The request
keeps a flattened map to detect presence and forwards None after validation.
Original1, the actual same configured clock, internal sampled-time APIs,
autonomous original2, ownership, recovery and receipt/hash domains are unchanged.
The existing isolated A6 supervisor controls its own clock and does not consume
this HTTP override; no study was modified and no new clock/harness was added.

## Observed results and attribution

- Actual147 RED, ebb8990de645da88186e84efe817a7b91830f7da: 1PASS/2FAIL.
  Explicit future asOf returned200, clock0, Full→Summary, text20→0.
  Legacy generation3→5; native4→6. Omission returned200, clock1 and kept Full/text/G.
  These are synthetic fixture observations, not general loss/security claims.
- 4c47c5aa: compile-only failure, private MetadataStore.db access in a new test
  helper (E0616 and dependent E0282). No corrected test/runtime result attributed.
- 8cf4640aca00b4f1bc8698a95db08e5c27e2e6db: compiled, 3PASS/2FAIL.
  Both actual override probes passed:400, clock0, Full/text unchanged, legacyG3→3,
  nativeG4→4. Omitted-time control passed for both formats:200, clock1, unchangedG.
  Two broader matrices stopped during snapshot setup with RepairAborted, before
  their request loops. This is separate from the actual override GREEN.
- 07681e18: test helper uses existing inspect_database_copy(..., true), repairing
  only its disposable copy of the open synthetic database. No production repair
  or ownership guard is weakened. Final results are recorded below.

The two compiled attempts each emitted ten executable artifacts, all hashed;
earlier progress messages calling this nine were a counting error. Manifests
are authoritative. Failed attempts remain individually attributable.

## Final validation on 07681e18

- Full all-features suite: **1205 library tests + 2 CLI integration tests PASS**;
  library run620.06s; four doc examples ignored, zero failed.
- Focused actual HTTP clock matrix:5PASS/6.80s; existing origin4PASS/4.08s,
  native compaction1PASS/3.67s, manual completion/failure3PASS/0.99s.
- cargo fmt --all -- --check: exit0.
- cargo clippy --offline --all-targets --all-features: exit0 with warnings:
  975lib,1041lib-test (939duplicates). No warning-free claim.
- Verified278 source/config files and all10 emitted executable hashes against
  the frozen manifests after checks. Driver exit0; its --rm container is gone.
- Tests ran once as a full suite on this compiled final candidate. Earlier
  compile/focus failures never started full tests. The sealing tail changes
  only documentation/evidence and does not require another local Rust run.

Exact commands, source/executable manifests and focused logs are in `after/`.
Full/clippy result excerpts preserve their raw-log SHA256 values. The separate
failed-attempt directories preserve diagnostics and source attribution; the
original `before/` remains the actual147 RED. Results/HOME copies are small;
no raw executable/cache archive is required or created.

## Focused coverage and limits

Five new HTTP tests exercise real legacy storage and actual native migration /
attestation, full router, actual adapter/runner, configured clock and publication.
Both field spellings cover null, numbers (including representable Chrono extrema
and u64 maximum), booleans, strings, arrays and objects. Literal duplicate keys
are sent as raw bodies so the fixture does not collapse them. Empty, null, array,
scalar, string and truncated bodies reject before work.

The no-work matrix compares canonical record bytes, namespaces, control,
persisted edges, raw receipt and ledger rows; tracker status and consecutive
marker-ticket IDs prove no accepted ticket between requests, with zero clock
calls. Closed admission still returns the input error. These are bounded fixture
assertions, not a whole-store concurrency or logical-state-hash claim. Snapshot
inspection uses an existing disposable-copy API while retaining the fixture's
engine read guard; no source database is opened a second time.

Omission and unrelated unknown fields return200 and sample once, keeping the
fixture Full and its canonical snapshot unchanged. Existing origin tests cover
manual1/autonomous2 phase loss and harddelete, retained old1 receipt bytes,
strict validation/reopen, and native physical compaction/readiness. Existing
manual-sweep tests retain known receipt failures and real publication completion.

## Validation environment and resources

Rust1.94.1, recalld-ci:1.94, Docker networknone, 2CPU/4GiB, jobs2; no home,
credentials, daemon socket or personal data mounts. One protected private Cargo
cache and target reused; no per-run cache clones. The source mount is read-only.
Package-only clean on the owned target preceded the original mount switch from
PR156; no other agent target was touched. Each run freezes source first and hashes
its emitted executables before tests. Full tests run only after focused success.

Raw root /tmp/recalld-public-sweep-clock-validation stays protected until PR and
results are recorded, then central cleanup may release it. Retain the reusable
/tmp/recalld-a3-matrix-target and /tmp/recalld-api1-lifecycle-validation/cargo for
next assigned work unless ownership is explicitly handed off. No raw HOME archive.

## Remaining scope

No ordinary native/public2 activation, new keyed sweep success/skip guarantee,
A3 completeness, quality/RL promotion, deterministic UUIDs or performance claim.
Global inherited-policy adoption remains Feynman's separate actual consumer.
Public routing and broader transport activation remain Huygens/integration work.
Published studies retain their original inputs/code. Exact-head CI and normal
review/publication are separate from local validation; no merge/deployment here.
