# Accepted producer roots

This child joins actual PR70 `af20d7331410feef8ea1d2a8803e6a0772bb5d99`
and PR78 `bec3bb5aa4e9525efed3170320e29c5413575d56`. It uses the existing
PR73 `WorkTracker`, owned by the same `MetadataStore`. It does not implement
GenerationRef publication stamps, a coherent read view, public whole-store
CAS, snapshot permission, or native-v2 mutation activation.

## Coverage

| Actual producer | Registration and completion boundary |
| --- | --- |
| Engine catalog create | One mutation guard before staged vector directory creation, through metadata receipt commit and prepared vector handle installation. |
| HTTP/MCP catalog adapters, including daemon forwarding | Root and worker child before `spawn_blocking`; worker passes the accepted parent into the engine. |
| Catalog policy update | Synchronous mutation guard; internal parent-taking entry point permits accepted descendants during close. Policy semantics and receipt identity are unchanged. |
| Selected text compaction | Same guard before new generation file creation, through durable selection, handle installation and cleanup attempt. Existing disposable validation and generation recovery remain intact. |
| Scheduled/startup/manual decay sweep | Startup registration before task spawn; manual registration before Notify enqueue; timer registration on tick. One coalesced manual notification owns one accepted root. Idle scheduling itself is not pending work. |
| Sweep descendants | Parented final graph/storage maintenance, ghost deletion, compaction, phase-index checkpoint and final derived completion. Expected record/policy and final graph projection checks remain inside graph then storage guards. |
| Standalone graph operations | Root before vector/entity/graph discovery or graph lock wait, through canonical edge application, graph/cache publication and acknowledgement. HTTP post-creation linking keeps one parent across all link stages; direct graph-node adapter operations register before their graph wait. |
| Single MCP store and benchmark store producer | Root before provider execution, inherited by canonical insertion and linking, retained through final publication/cache work. Existing batch calls inherit their reserved parent rather than creating unrelated roots. |

New roots fail admission during close. Existing accepted parents admit children.
Each queued mutating worker owns a child before enqueue, so cancellation cannot
make a detached worker invisible. Cancellation, unknown outcomes and unavailable
owners retain unresolved work. A scheduler shutdown abandons an unexecuted manual
ticket; it does not mark that ticket completed. Known returned failures may
finish their execution tickets; durable pending
effects still block capture and can be retried by the same owner.
Durable outbox/purge/batch checks remain independent requirements for capture.

A sweep's original committed receipt or unknown operation identity takes priority
over a later derived-publication error. The receipt is not reconstructed from a
string. Compaction errors and missing completion handles are recorded rather
than treated as successful publication. No root is finished just because a timer
or enclosing future stopped. Successful execution finishes only after its
publications; a known publication failure does not claim successful readiness.

## Necessary join adjustments

The two parents need real `MutationGuard` reconciliation in observed exposure and
feedback preparation, a fresh `lineage::ensure_serving` check under that guard,
and the same maintenance guard through `finish_record_effects` and `apply_pending`.
Keep `BatchChild` as the final optional argument. Batch receipt outcomes stay in
the exhaustive observation-validator match; the actual batch validator remains
mandatory. These match the independent PR80 `6ccb480` glue semantics, without
adding PR77 or PR80 ancestry to this producer child.

Scoped forget retains its final namespace assertion inside the storage write
critical section. Derived completion retains the full committed receipt on
failure. Observed, recordAccess and protected-scope wrappers from PR70 are
preserved, as are PR78's batch replay, pending-item scan and accepted lineage.

## Remaining participation boundaries

This is not a whole-core admission proof. The HTTP creation handler still invokes
its provider before its insertion adapter root. Legacy exposure/feedback queued
workers, complete true/prepared query roots, and cache/query read-view coherence
need the consumer-owner join. Halley owns those optional-parent consumer seams
and the publication registry/coherent views. False-only coherent queries alone
do not cover true-mode callers. Native catalog/phase/purge and native batch
participation belong to their actual writer/proof owners; this branch does not
invent receipt variants or relax native proof validation. No inactive fixture or
raw test helper is evidence of public coverage.

Shutdown's synchronous flush and phase checkpoint are not an accepted asynchronous
queue: the checkpoint enters the same metadata mutation guard. This change does
not turn legacy shutdown timeouts into proof that all tasks stopped.

## Validation

Frozen source: **906 library and 2 CLI tests passed**, with 4 doctests ignored.
All fourteen added regressions are included in that full run. Formatting passed;
Clippy exited 0 (554 library warnings, 614 test warnings including 541 duplicates,
one binary warning). The 198 runtime files match before/after SHA-256 manifests.
Tests used read-only source, Rust 1.94.1, offline Docker, two CPUs, 4 GiB, two
build jobs, and private source-specific cargo/target directories.

The initial validation wrapper completed the full suite, then exited 101 because
it requested the absent `offline_eval` example. That log is retained. This
70+78 child does not add an unrelated parent to acquire the example. Remaining
format/Clippy checks ran separately on the unchanged source; the passed full
suite was not rerun. Exact commands, source/binary hashes, integration adjustments
and development-log limits are in `docs/evidence/accepted-producers`.

The tests exercise actual catalog file/handle boundaries, selected compaction
and reopen, adapter queues, standalone edge discovery/publication, provider
execution, sweep shutdown/cancellation, and transitive deletion/compaction/view
completion while root admission is closed. They are correctness experiments on
synthetic tempstores, not performance measurements or power-loss tests.
