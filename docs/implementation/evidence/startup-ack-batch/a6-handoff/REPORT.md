# Pilot001 abort and one startup-only diagnostic

**No retrieval-quality result.** Original de8391 pilot stopped at candidate startup before completing any training pair. One separately approved instrumented startup-only run observed sustained acknowledgement progress and reached the same60-second wall deadline. Both failures remain failures; no original candidate replacement, pre-drain, longer timeout or retry. Hume now owns a separate startup-ACK production-fix source plan; any future fixed-candidate study needs an explicit amendment.

## Original frozen pilot

Source `de8391a07b9ea2454b8195cb99e39b6709d9167d`; original runtime5aa4692c7d326d2ef10712939eb01484a520e891 versus coherent096195d93528a0401e4d80bb7d6a87cfc5a2955f. Coordinator approved02:09–02:24UTC,2026-11-16, referencecoordinator-train16-de8391-20261116T020900Z. Only that receipt was filled in the reviewed manifest:SHA3b2275fd25575d364b2ce83a921af9328bbb43eff1e41cc31014f5fcaaf1f13e. Both actors2CPU/4GiB/tmp128MiB/network-none/current-question-only; no future schedule, labels or ambient credentials mounted.

Actual run63.218272542seconds, no overrun. Scheduled38 actions;1passed,2attempted,1failed,36notattempted (remaining4negatives and all32primary actions). Stop-on-first-failure worked. Source/binary/seed hashes were verified before admission. Baseline successful clone was retired after validation; candidate failed clone remains. `summary.json`, `ledger.jsonl`, `schedule.json`, driverexit1 and scoped raw observations are retained beside this report.

| Action | Actual observation |
|---|---|
| Baseline negative-empty | Childexit0,1.571841s; constructor5,183rows equal, namespace1;1actual replay request; actual FTS success/0hits/errornull; group/drains/replay joined; accepted. Constructor preflight30,273us, startup1,030,912us, after-scan2,470us. This is one negative wiring result, not an evaluated training pair. |
| Candidate negative-empty | Child60.477928s,whole-child60s wall timeout,exit-15;0constructor records,0provider,0FTS,no response; group gone/drains/replay joined; raw result saved before acceptancefailed. Peak214,507,520bytes;CPU60,096,434us (user56,250,742/system3,845,691);0throttled periods/0OOM. |

Candidate container ran02:09:30.523735105–02:10:31.111405357UTC. Both owned containers and driver were confirmed retired; coordinator ended quietwindow early. No unattempted negative was called an empty result and no successful-pair intersection was scored. Baseline's raw response contains public corpus payload and remains local; its hash/projection is retained instead of redistributing dataset text. Candidate raw result has no response and is copied intact.

Read-only failed-store stat/hash inventory opened no database or repair path. FTS DB/WAL files exist and were modified early; metadata changed later; vector/selected text bytes match seed. These facts establish some startup progress, not a completed FTS rebuild, canonical corruption or exact pending count. See [DIAGNOSIS-AND-PLAN.md](DIAGNOSIS-AND-PLAN.md).

## Source hypothesis and exact reviewed overlay

Frozen096 `system.rs:810–821` rebuild_all leads to `derived_completion.rs:432–434` actual acknowledgement. `storage/metadata/effects.rs:341–347` snapshots pending operation IDs and loops; each per-ID acknowledgement calls pending_effects at1052 and1071. pending_effects validates/decodes remaining rows and matching receipts and sorts (98–304,355–356). For N completed acknowledgements without concurrent insertion, those two scans alone entail N² row visits plus the initial scan. That is source analysis, not measured scaling or scan-versus-commit attribution.

Hume independently reviewed the exact patch and coordinator approved one build plus one run. Only the candidate's two study-adapter files and actual acknowledgement-batch observer were changed. Existing guards, ACKalgorithm/order, transactions and ranking stayed intact; no additional database reads or per-effect timing. Coarse synchronous stage events and at most25 sparse batch events at this population use existing results/ID-vector length. Startup-only returns after constructor guards and before query dispatch. The separate worker admits only diagnostic mode with empty cache, retains the60s wall deadline/drains/group retirement, and fails on any provider/FTS query call. Original frozen worker is unchanged.

| Pin | Exact value |
|---|---|
| Original failed candidate executable |`526ba1ee48d112d134be9532892b8795569c12ad71b2a1db6ac1264134514a47` |
| Original two-arm source export |`f8a7e82392d2a78731a202ae6e29eee06aebe98935c474bf9ef4ca4fb1d80642` |
| Diagnostic patch |`c0c07a9d6ad34f9a2fb562ff20a2823e1f6f7fa509c40940221a0d3f4ad2ab46` |
| Diagnostic worker |`84ffbd9d54b34282dc7bc5b29e9cc350190de65300299db1a0cdc3861b4daf78` |
| Diagnostic complete source manifest |`8f1321e30a68f8eae9873ab35d75a6c06887c8fc73af1a0f6a9a894996f492c8` |
| New actual diagnostic executable |`26984986969f1f11ac4d5ee9782eecab4fb77ff029b9bb8140675c0ad6441836` |
| Accepted closed seed |55,165,669logicalbytes,6files; exact unchanged file hashes in execution-pre.json and prior full-preparation lock |

One release example build passed122.278191s,2CPU/6GiB/offline/locked/-j1, same existing target;peak3,955,113,984bytes,0OOM;95existing warnings. No broad suite/model/seed rebuild. Original and new binaries are separate retained files; original source/seed were never patched or pre-drained.

## One actual diagnostic outcome

Fresh clone,2CPU/4GiB/tmp128MiB; no quietwindow requested. Container02:21:42.050217127–02:22:42.253788322UTC. Child60.111694s/exit-15/timedOuttrue; host60.451634s,total including clone/verification60.632184s. Peak220,303,360bytes;CPU60,002,096us (user56,437,625/system3,564,471),602periods,0throttled/0OOM. Group gone/drains/replay joined;containerremoved/driverexit1. Original/new binaries, source, worker and accepted seed hashes all unchanged; diagnostic clone retained. No provider requests, FTS query observations, constructor completion or response.

Adapter-local events show preflight_open147→301066us (300919us), canonical_before301112→307802us (6690us). Version-open-local with_clock begin exists, with no end. The actual ACKbatch begins and captures total5,183 from its existing IDs vector. Its own local elapsed clock reports:

| Successful completed ACK lower bound | ACKbatch-local elapsed seconds |
|---:|---:|
| Planned5,183 |0.015649|
|1|0.043989|
|256|6.840740|
|512|13.294725|
|768|19.332813|
|1,024|25.161888|
|1,280|30.670768|
|1,536|35.718266|
|1,792|40.457027|
|2,048|44.811737|
|2,304|48.851938|
|2,560|52.570064|
|2,816|55.916391|

No ACKend/with_clockend/complete_pending_effects/after-snapshot/constructor-completion event followed before termination. At least2,816 actual acknowledgements succeeded; more may have completed after the last sparse event, so neither exact final count nor exact remaining count is known. The observed timeout is localized to a startup path that entered and made sustained progress in the active acknowledgement batch. It is not evidence of a stall before ACKentry.

There are three elapsed origins: adapter, version-open and each ACKbatch. Subtract only within the same scope; never sum nested intervals. ACKend is emitted before finish_work and would not itself prove drained work or readiness—even though no end occurred here. The source has repeated full validation, but this run does not isolate scan/decode versus commit/sync costs, prove empirical asymptotic scaling, support extrapolated completion time, or compare performance ratios. Shared-host contention and instrumentation are disclosed. No retrieval score or production optimization is claimed.

## Handoff, reproduction and retention

Hume owns the separate startup-ACK fix plan, coordinating actual minimal parent/effects ownership with integration. Preserve caller/publication/work-ticket semantics, unknown outcomes, durable receipt/canonical/readiness guarantees and regression coverage in that separate review. This report does not prescribe or implement a batched-ACK shortcut, suppress validation, or authorize another study run. A fixed candidate must be separately named/amended; original096failure remains part of the before/after record.

Exact patch/worker/source pins, compact actual logs/counters/commands, raw stage events and source/build provenance are retained under this directory. No dataset/model bundle or raw corpus response is redistributed. Protected local roots: empirical pilot-001 (failed clone), startup-diagnostic-001 (source/new binary/failed clone), storage-full-002 closedseeds/originalbinaries/failedseed, preparation-full-001 failed-format outputs, preparation-001 installedsite, prefetched assets and solecache/sharedregistry. No archive prerequisite for unrelated completedPR cleanup; these active research inputs are not released. Centralcleanup remains528successfulactions through65; pending owner releases are separately queued and require fresh guards.
