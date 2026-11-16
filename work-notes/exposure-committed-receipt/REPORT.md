# PR181 exposure postcommit receipt correction

Runtime: `1f6a80c3059dbc11390c18bb95bfb812d093423b`.
Sole base: frozen queue composition `01dc414e311b2a35aed35dd443cfe100ce80563a`.
Branch: `fix/exposure-committed-receipt`; isolated worktree:
`/tmp/recalld-implementation/exposure-committed-receipt`.
Halley owns final composition, full validation and merge. This child was not
pushed and did not edit the frozen queue or any builder branch.

## Confirmed failure and correction

The actual native exposure transaction commits before
`publish_vector_validation_commit` publishes the owned validation position.
That publisher already marks its owner unavailable and returns the exact durable
receipt in `StorageError::CommittedMutation` on failure. The observation mapper
previously recognized only a top-level `CommitOutcomeUnknown`. It dropped that
receipt into diagnostic text instead of the structured receipt list.

Actual RED source `2fc4695c` executed six cases, all failing at the missing-receipt
assertion **after** strict reopen verified durable generation, history, complete
receipt identity and the original API request hash. API1 and API2 first-group and
QueryEngine failures reported `notCommitted` with zero structured receipts despite
G+1. Later-group failures retained one preceding receipt despite G+2 and two
persisted exposure receipts. `RED-excerpt.txt` retains the diagnostic boundary.

The production change is confined to `ObservationError::storage`: peel typed
`CommittedMutation` wrappers, append their in-hand full receipts after prior
namespace receipts, and deduplicate only full equality. The terminal typed cause
sets unknown precedence: `CommitOutcomeUnknown` retains its actual operation ID;
`WorkerOutcomeUnknown` retains no invented ID. The message contains the leaf cause,
not duplicated serialized wrapper receipts. No lookup, receipt reconstruction,
request-hash change, new durable field, publication change or readiness relaxation.

The cfg(test), instance-local AtomicUsize hook fails the Nth nonempty owned-vector
publication inside its existing postcommit result closure. It uses the unchanged
production known-commit wrapper. It does not reuse the lost-ACK hook, inject a
precommit failure, tamper with a database, or simulate a process crash.

## Validation and attribution

All executed checks used synthetic data in `recalld-ci:1.94`, Rust 1.94.1,
`--network none --cpus 2 --memory 4g`, `CARGO_BUILD_JOBS=2`, no home/credential/socket
mounts. Source was frozen per candidate. The same private Cargo/target was reused;
only this package was cleaned on source-root switches. No dependency cache clones.

- `e84eab9a`: compile-only failure, four inherited E0308 test assignments; no tests
  executed. Halley approved four test-only `FlatVectorIndex::new(5/4)` to
  `VectorIndexOwner::uniform(5/4)` replacements, separately committed `73e486e9`.
  Wrong-width failure and restored-width replay assertions remained unchanged.
- `b58f0871`: six test setup failures, missing vector in the new second-namespace
  legacy insert. These did not reach the fault hook and are not defect proof.
- `2fc4695c`: valid vector supplied; actual RED, **0 passed / 6 failed** as above.
- `1f6a80c3`: **32 distinct focused tests passed**, no ignored tests in these runs:
  eight new tests, 11 existing native exposure/origin tests, seven legacy observer
  tests, six existing replay/query-projection/publication-failure tests.
- An initial legacy filter matched zero tests. `legacy-observer.log` remains
  explicitly zero coverage. The corrected `decay::observation::tests::` tail ran
  seven tests against the **same sealed GREEN executable**, without compilation.
- `cargo fmt --all -- --check`: exit 0.
- `cargo clippy --offline --locked --lib --tests`: exit 0 **with warnings**:
  1211 lib, 1298 lib-test (1150 duplicates), one bin-test and one integration-test
  warning. This is not a warning-free claim.

No child full suite was run. Halley retains the exact composed-head full/CI gate.
`EVIDENCE.json` records exact source IDs, per-run manifests/log hashes, binary hashes
and every executed test. `TESTS.txt` contains the 32 distinct test names.
`changed-files.json` supplies all seven file base/runtime SHA256s for hunk review.

RED executable SHA256:
`94a2085a24fc71109cb84facf922112b9c5264b6ba16297bcf5849f82a621bfd`.
GREEN executable SHA256 (also the corrected seven-test tail):
`ad1bd786bac1626ca187a0ae4f1469474c5cd63915c54209473068454eced645`.
Source/config/data/document snapshots each contain 8666 tracked files; this count
is not described as 8666 Rust files. Raw failed source/log/executable provenance
remains separately retained, with mtime and size in the evidence record.

## What the tests establish

First/later groups under both original API versions retain ordered, exact prior
and current receipts. Namespace ordering and returned-ID dedup use the real
DerivedCompletion consumer. Actual QueryEngine observation fails the whole page;
the actual prepared API2 query consumer returns 503, no data, no readiness success,
full observation receipts and MCP isError. Public API2 activation is unchanged.

The poisoned owner rejects completion and even an older-target barrier, and leaves
accepted work unresolved. Strict reopen validates G/history/receipt proof. A keyed
internal exposure replays after reopen without a new clock sample, generation or
record/history change; changing its original API version conflicts. This is not a
public keyed-query guarantee. Nested-wrapper/unknown assertions use actual durable
receipts but are typed-mapping tests, not a claim of an executed nested storage
failure. Existing actual unknown-ACK, cancellation, later-precommit failure and
publication/replay tests remain green.

Huygens and Feynman reviewed exact runtime `1f6a80c3` read-only and found no source
blocker. Their ACKs cover contract/source semantics, not independent execution.
No schema/proof weakening, broad refactor, new scheduler, ordinary activation,
public routing change, full A3 completion or performance/quality claim.

## Ownership and resources

Pascal owns the isolated child until the exact READY/FROZEN handoff; Halley owns
queue integration/full/merge. No further branch edits or pushes after handoff
unless ownership is returned. All owned validation containers have exited and
were removed by `--rm`; a name-filtered docker ps was empty at drain reporting.
No other service or container was stopped.

Protected pending queue acceptance/results retention decision:
`/tmp/recalld-exposure-committed-validation` (failed and successful source/log/binary
evidence). Central cleanup was notified; no deletion or cleanup bytes claimed
here. Retain one reusable `/tmp/recalld-a3-matrix-target`, private Cargo
`/tmp/recalld-api1-lifecycle-validation/cargo`, and shared read-only registry
`/tmp/recalld-unit-tests-l__ja1yo/cargo/registry`. No new cache copies or HOME raw
archive. Preserve the worktree/code/Git/report. After acceptance/publication and
recorded results, release the disposable raw root explicitly to central cleanup.
The primary-study scheduling drain is respected: no new heavy work after this
completed proof; only compact report/source handoff work remains.
