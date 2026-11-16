# Native selection demand

## Scope and source

Actual single and reserved batch insertion now derive auxiliary selection demand
from the existing frozen `NativeStoreIntent`. Entity/temporal-only selection does
not read candidate vectors; all auxiliary channels off avoids the namespace
candidate scan. Insertion's original profile/numeric validation still executes.
Uncomputed similarity is `None` and cannot be consumed by semantic/advisory code.

Parent: PR158 `9c90f8201d476918077d3d08920fdb989269c205`.
Plan: `19428d78`; original BEFORE: `677d4ecbc27ef0c4c095ca79ef0085370ba6e83a`;
corrected BEFORE: `f342fb53` (full source ID in ITERATIONS.json).
Runtime: `36bd78c8`; final test/source:
`6b7f0765572caa3ab69a1886357d1a741dcac1cc`.
Later source commits only correct test assertions/fixtures; the three production
files are byte-identical to runtime36bd. No extra implementation parents.

Feynman ACKed exactly `graph/native_selection.rs` and the capture-call arguments
in `derived_completion/native_store.rs` and `mcp/native_store_batch.rs`. His
provider seal/certification work is not included. Forward-port replaces each
`capture(&engine, intent.insert_intent(), &generated)` with
`capture(&engine, &intent, &generated)`; preserve his actual generated result.

## Executed work counts

Each actual consumer uses two eligible canonical candidates, executes all five
modes, verifies exact edge targets/kinds/weights, then strictly reopens the store
and replays the full receipt under unavailable provider and changed settings.
The two tests use current-thread runtimes and thread-local test-only counters at
the actual selection scan/vector-read calls. Completion/rebuild/startup reads
are outside this measurement. This is no wall-time or throughput claim.

| Mode | Corrected BEFORE scans / candidate vector reads | AFTER scans / candidate vector reads |
| --- | --- | --- |
| All auxiliary off | 1 / 2 | 0 / 0 |
| Entity only | 1 / 2 | 1 / 0 |
| Temporal only | 1 / 2 | 1 / 0 |
| Semantic only | 1 / 2 | 1 / 2 |
| Advisory only | 1 / 2 | 1 / 2 |

The corrected BEFORE completed edge and reopen/replay assertions before each
test's final aggregate count failure: **0 pass, 2 fail**, 9.36s. Those positive
assertions are not represented as two passing tests. AFTER both consumer tests
pass. Checked-empty versus disabled advisory, current visible hydration, phase,
scope and UUID tie controls also pass in the five existing selection tests.

Additional controls reject using uncomputed scores for semantic/advisory work,
reject invalid insertion vectors and absent provider identity even with all
auxiliary channels off, and preserve G/clock on those rejections. Real single
and batch provider probes corrupt a canonical candidate reference after provider
entry. Semantic and advisory consumers still reject; no child receipt is created.
Single G stays unchanged. Batch keeps the exact reservation receipt and reserved
child ID, reports `Unknown/RECONCILIATION_REQUIRED`, and retains the durable child
as `NotAttempted`; it does not invent a completed/rolled-back child. Only parent
reservation advances G. Corrupted stores reject strict reopen.

## Validation

Final source6b7f: **4 demand/control tests pass** (15.80s), **5 existing selection
tests pass** (4.52s). Full all-features: **1,336 library pass, 0 fail, 2 ignored** (720.05s),
**2 CLI pass**, three zero-test binary targets and four ignored doctests. Fmt
and all-targets/all-features clippy exit0 (warnings retained). Final cgroup peak
3,982,888,960 bytes, max0/oom0/oom_kill0. All owned containers exited and were
removed. All338 recorded source inputs remain unchanged. Final test executable
SHA256 `36ba05e3c1a09c6bda846613ae94e1ce3480e2d7420f40999ce859a5f42b577b`
matches the focused run; `VALIDATION.json` and `executed-binaries.sha256` retain
full log/image/executable attribution. No extra full run after documentation.

Docker `recalld-ci:1.94`, Rust/Cargo1.94.1, network none, 2 CPUs, 4 GiB,
`CARGO_BUILD_JOBS=2`, dev/test debug=0. Only assigned source, private cargo/target
and evidence mounts; no HOME, credentials, socket or personal data. All fixtures
are synthetic temporary stores. Commands: `cargo test --offline --all-features
--lib selection_demand`, same with `native_selection`, then `cargo test --offline
--all-features`, `cargo fmt --all -- --check`, and `cargo clippy --offline
--all-features --all-targets`. This ancestry has no examples/offline_eval target;
none was added solely for validation.

One private cache/target was reused. Every source-root switch ran
`cargo clean -p recalld` in that owned target before rebuilding: initial158-derived
source, corrected BEFORE, then final6b7f source. Source manifests and executable
hashes distinguish the corrected BEFORE and final executions; no warm-binary
transfer is claimed. Source bytes are verified unchanged after each final run.

## Retained failed iterations and attribution correction

- Original677: single ran five-mode positive edge/reopen/replay assertions and
  failed the excess-read count. Batch failed earlier because `createdAt` is not
  accepted by this batch boundary. An early progress update incorrectly called
  both failures read-count proofs; corrected explicitly before further work.
  Original raw-log hash and excerpt remain recorded unchanged.
- Corrected BEFOREf342: remove unsupported wire field and align the genuine
  peer/single creation time with the injected final clock. No production change.
  Both actual consumers then reach and fail only excess-read count assertions.
- Runtime36bd focused build: test `unwrap_err()` imposed an unsupported Debug
  bound on the success type. Compilation failed; no test executed. Any available
  old binary in that directory is explicitly not evidence for36bd.
- Test-only0e976: 2 controls pass, 2 fail: the original invalid batch fixture and
  a corruption assertion expecting a raw diagnostic instead of the existing
  structured batch status. The log/source are retained. Its executable SHA was
  not captured before the later owned target clean; this intermediate run is not
  used as exact-binary final proof. No rerun is relabeled as the original run.
- Final6b7f fixes only those test expectations, asserts the precise pending child
  and parent identity, and leaves all production code unchanged. Final focused
  proof and subsequent full run use this independently rebuilt source.

## Limits and cleanup

No certification authority, profile identity, persisted proof/hash/codec or
public activation changes. Metadata-only selection is not a global vector-byte
integrity scan; existing owned startup/offline validation and actual derived
completion remain unchanged. A future certified consumer must still use
Feynman's validated owner interface; this slice cannot certify old generated
vectors. Process/reopen tests do not establish power-loss durability. No A1
backend choice, complete writer participation or full-spec acceptance claim.

Active raw: `/tmp/recalld-implementation/native-selection-demand/.validation`.
Registered corrected-BEFORE worktree is source, not disposable cache. Central
cleanup alone removes completed raw after publication/results are recorded; no
new raw HOME archive. Preserve tracked reports/source/.git/worktrees. One owned
cache/target under `/tmp/recalld-curie-active-build` is retained for this run and
the assigned later certification consumer, subject to explicit cleanup handoff.
