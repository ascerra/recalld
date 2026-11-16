# A5 legacy batch producer counters

Local correctness validation complete; not a publication or performance result. Direct prerequisite is PR165 `3cfa164453d5e1c0f149e123acbc2331eca384ab`. The approved pre-code plan is `9514ac59cac58de176544869c80837a5558a16ab` in [legacy-batch-counters-plan.md](legacy-batch-counters-plan.md). Halley, Huygens, A6 and Feynman acknowledged their actual shared seams before runtime changes. No native/certification/checkpoint ancestry or new registry.

## Measured facts and limits

The existing MCP durable batch adapter supplies one inherited request decision through reservation, actual pending-child dispatch, rejected-child bookkeeping and final canonical-progress reconciliation. The real stdio and daemon transport roots continue through actual response write/flush. Direct typed-adapter tests are controls, not wire benchmarks.

`batch_input` counts supplied items by fixed operation category. `batch_reservation` reports typed committed/replayed, unknown or error facts. A successful replay returns a retained receipt, not another commit. `batch_child` attempts count only pending children actually dispatched; completed replay inventory is not another attempt. Returned child branches distinguish committed, committed-unready, unknown, final rejection and provider-unavailable/reconciliation-required. `batch_bookkeeping` reports returned progress or unknown/error; returned progress can reflect a concurrent canonical result and is not a new-commit count. `batch_progress` measures the existing healthy-handle lookup, with no new work ticket or completion barrier.

`batch_outcome` describes the actual returned item totals/ready flags, retained child/bookkeeping receipt inventories and unknown-bookkeeping inventory. All-invalid input can commit its parent while dispatching no child. Item readiness is not the `all_ready()` predicate: that predicate also requires zero rejected items. Root completion/abort does not establish that accepted descendants drained. No global batch-atomicity or rollback claim follows from a trace. An `ok` request duration means the Rust call returned `Ok(BatchOutcome)`; it does not convert partial/unknown item statuses into committed or ready successes.

The `commit_call` hook in `metadata/batch.rs` measures only the actual `tx.commit()` call. Fixed resources distinguish parent reservation and rejected-child bookkeeping; ordinary child effect commits retain their existing hook. A raw commit success can precede an injected caller-visible unknown outcome. Opaque backend sync counts remain unknown; this change adds no sync call and does not infer fsync zero. Existing explicit Rust sync wrappers are counted once at their actual site.

Workers reuse the captured dispatcher/span/decision. Queue delay ends at closure entry; lock wait precedes acquisition and hold ends after releasing the original guard. No ticket, canonical receipt, replay identity, hash, lock order, transaction or durability change. Timeline marks/residual partition their own parent only; nested child/lock durations are not additive. Diagnostic traversals and field evaluation are gated before execution, bounded by admitted batch size. Disabled/unsampled paths capture no active trace state and sample no instrumentation clocks. User/provider text, arbitrary errors and caller keys are not new labels. Existing isolated receipt identity traces remain unchanged.

## Preserved attempts

- `7e9fb6fbea78dac693083071b85591953dccb55f`: fixture omitted required `storeId` for a keyed request; no provider invocation. Not the intended instrumentation RED.
- `17687d53fdd77e73cb64aa56eedd0a577da074bb`: actual partial outcome reached, but fixture incorrectly required `all_ready()` despite one final rejected item. Not the intended RED.
- `1fa2752519468f08752ea33a0407057e8ede5d14`: genuine missing-instrumentation RED. Actual partial MCP input, provider call, canonical G+2, parent/child receipts, final ready flags, publication and flush assertions passed; then `missing actual batch reservation diagnostics` failed. Library executable SHA-256 `2eeba586edf43d922423253174f2fba997f81c136256d037f1ce4ce2b59b6b72`. Inside-container pre/post source verification passed, exit101.
- `f08fb6f2183b589bbf17ac412ee03da2b2deae50`: fixture compile error accessing a private adapter provider field. No runtime result. Corrected through the existing adapter constructor in `e320ecb7f7e16b5949759cd09295f728ac327c2b`; production unchanged.

- `e320ecb7f7e16b5949759cd09295f728ac327c2b`: 7 focused tests passed, 3 fixture expectations failed. Two aggregate commit counts omitted actual existing `edge_effects` commits alongside record metadata commits; one compared the returned replay flag as if it were unchanged. The correction asserts separate exact counts for reservation, rejection, metadata effects and edge effects, and equality of every receipt field with the expected `replayed=true` return flag. Production instrumentation is unchanged in `30abe0d6e5029fb483aa5c70f8f2db3f94d75e26`.

## Focused validation

Frozen source `30abe0d6e5029fb483aa5c70f8f2db3f94d75e26` passed 10 tests selected by `legacy_batch_` (including one existing forget behavior test) and 27 selected by `performance::tests`; these sets overlap and are not 37 unique tests. Runtime assertions used default stacks, no boxing/stack-size workaround. Actual stdio partial delivery and failed flush, daemon framing with explicit vectors, all-invalid parent-only work, keyed replay/resume, failed second provider call, rejected-child bookkeeping, current committed-but-unready receipt, actual raw-success/caller-unknown boundaries and reopen recovery passed.

The queued reservation test occupies the only blocking slot, waits for the actual parent and child work tickets, aborts the root, verifies pending/unresolved work, then releases the original worker. The parent reservation commits afterward under the original trace context; no child dispatch occurred, and worker success does not settle the dropped root. This is a deterministic reservation-boundary proof, not exhaustive cancellation at every instruction or proof of a globally drained batch. Existing165 owned-publication-worker abort tests remain included. Unknown-commit fixtures use actual existing injection stages and require reopening before canonical reconciliation; no unsafe same-handle readback or synthetic storage outcome.

Selected/unsampled actual batches use controlled inherited decisions; the nested wrapper never calls its sampler again. The disabled diagnostic primitives assert zero allocation, no active clock and no field evaluation. These do not assert zero application allocation or eliminate opaque backend work. Existing sampling/redaction/read-only/sink-I/O tests remain included; arbitrary subscriber panic is not handled or claimed.

## Final frozen validation

Runtime/test source **`30abe0d6e5029fb483aa5c70f8f2db3f94d75e26`** preserves production instrumentation from `f08fb6f2183b589bbf17ac412ee03da2b2deae50`; subsequent Rust changes corrected fixtures only.

- Offline `cargo test --all-targets`: **1,048 library tests**, **2 CLI tests**, **4 example/harness tests**, all passed. Local scope is all-targets, **not all-features**. Library duration 419.91 seconds; whole final container 513.268543 seconds. These are validation durations, not service metrics.
- `cargo fmt --all -- --check` passed. `cargo clippy --offline --all-targets` exited zero with warnings: 702 library; 771 library-test (683 duplicates); 1 binary warning duplicated in its test target. Compilation reported 27 library and 10 library-test warnings (6 duplicates). No warning-free claim or unrelated cleanup.
- All **232 committed exported files** matched inside the container before formatting/compilation and after execution, then against the host immutable snapshot, current working source and exact Git blobs. Eight executed target hashes are recorded. The full run used a fresh atomic snapshot and package-only cleaning of this agent's sole target; no source edits during execution or stale source-root switch assumption.
- Rust1.94.1, jobs2, network-none, two CPUs and 4 GiB, image `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`. No home/credential/socket mounts, host-data tests, stack override or exclusive-host claim.

[Validation manifest](evidence/legacy-batch-counters/VALIDATION.json) SHA-256: `4781a008442b17dae17792e42870018b870be05836611e176125bb21d4284406`.
[Source manifest](evidence/legacy-batch-counters/source.json): `5da31a4ffb4f6565e01bbc7e93fdd5846f768bb10d05dd5b570e50b3e705ac97`.
Full log: `27e5b98589a0a2d5a3e6f3b0bb0afc82a4939d06c7af9d37355089e82f01ee7c`.
Library executable: `395317225c63b9c4dc5cf5aad97f7f6be22899a8fa07cc48fa8b90f01a686e26`.
[Compact outcomes](evidence/legacy-batch-counters/RESULTS.txt) preserve failed attempts; [commands](evidence/legacy-batch-counters/COMMANDS.md) and [inside checks](evidence/legacy-batch-counters/validate.sh) bind the procedure. No raw data or executable is in Git.

## Remaining gates

Exact-head publication review/CI and later integration remain separate. Halley reported a separate current-main `c65d8136` PR165 daemon transport-input assertion failure (12/13 focused pass); isolated/parallel same-binary passes do not clear that original failure. The relevant handler/protocol/performance/test source is unchanged versus165 in that join. This branch's same daemon fixture and full suite passed, but neither fact establishes the cause or fixes the integration failure. Halley retains the failing executable and is adding captured-row diagnostics; no frozen branch/source change or speculative workaround is included here. No performance measurement, quiet window, default enablement or promotion claim. Existing full-trace/sampled/incremental failed or unmet gates remain unchanged. Native batch/single/certification consumers, uninstrumented background/provider-internal work, complete external request denominators and actual syscall attribution remain separate owned work. Huygens's API1 surface split and Feynman's certification pre/postcommit hooks require their actual future joins; this branch neither imports nor claims those consumers.


## Changed source and retention

Production changes are limited to `src/mcp/batch_adapters.rs` and the actual commit-call block in `src/storage/metadata/batch.rs`. New regressions are in `src/performance_batch_tests.rs`, referenced from the existing `src/performance_write_tests.rs`, plus the actual private daemon socket fixture in `src/daemon/producer_tests.rs`. The plan/report and compact text evidence are documentation only. No public schema or shared sampler/RequestDrop implementation changes.

All seven owned run containers have been removed after preserving exit state; no owned validation container remains. The seven raw roots under this worktree's `.validation/legacy-batch-{red,red2,red3,focus,focus2,focus3,green}` stay protected until resulting PR and recorded-results handoff, then are released to the central cleanup owner. No raw archive, executable or build cache is staged or copied into HOME. Source/worktree/Git/reports remain protected. The single private cargo/target named in [commands](evidence/legacy-batch-counters/COMMANDS.md) is retained for the next owned task. Actual net disk recovery is not measured.

Compact results are appended to the actual notebook `/Users/caleb.evans/work-notes/work/Experiments/Recalld correctness and retrieval research.md`; no raw HOME copy is required or created under the current user retention override.
