# Native API1 purge: bounded full-receipt consumer

Status: local validation complete; awaiting final source/report publication review.
Not published, no native startup/public2 activation.

## Source and dependency

Actual sole base: published176 `1f5d083d8142dd631f16f95617d23fddc85751ad`.
Runtime `369d8e9219441a031b5502edd3d1ea435334c576`; final test source
`90938aa5` adds only test assertions after the010 source
`8c990c010355243863e7f2bd4865e516b73efa63`. No Cargo/features/examples/backend changes.
The branch is `impl/native-api1-purge-consumer`; no parent updates or broad joins.

The old native API1 producer completed canonical purge but lost typed evidence
on failure. The replacement uses the existing purge/completion owner and accepted
work. It preserves the entire verified matching receipt and actual evidence scope,
unknown outcomes, and measured physical counts. A matched child is not relabeled
as final merely because a separate read shows Complete. Foreign pending receipts
stay in the typed internal cause; only actual matching purge evidence is projected.
Generic known progress/storage errors use existing SERVICE_UNAVAILABLE/-32006
with committed/fullreceipt/scope, without invented readiness. MetricsUnavailable
requires a validated final matching receipt and explicitly lacks physical observations.

Actual1000-row execution exposed a second problem: a full child receipt can exceed
the old1MiB frame and the daemon falls back to -32005 without evidence after commit.
Fresh native requests now use the real peer's existing1MiB/negotiated8MiB capacity.
Inside the same guarded start transaction, capped raw name resolution and conservative
union counting refuse an operation that cannot prove all required receipt frames fit,
before any canonical start or owned ID draw. No durable permit, chunk rewrite,
receipt reference, truncation, automatic new-key/name retry or new protocol was added.
Legacy deletion/default trait behavior is unchanged. Direct local MCP receives no
invented daemon frame cap. The provided Rust NamespaceRegistry method is additive;
custom implementations retain default behavior and are not certified by these tests.

## Changed runtime surfaces

| Files | Purpose |
| --- | --- |
| `src/daemon/server.rs`, `src/mcp/bridge.rs` | Carry the actual negotiated peer capacity through a provided trait seam; retain legacy behavior |
| `src/mcp/bridge_adapters.rs`, `src/mcp/native_purge_api1.rs`, `src/mcp/tools.rs` | Actual native API1 dispatch, typed full-receipt/error projection, strict remote payload validation and bounded frame envelope |
| `src/derived_completion.rs`, `src/derived_completion/native_purge.rs` | Existing accepted-work completion records physical observations, preserves known/unknown evidence and retains worker-owned publication lease through cancellation |
| `src/storage/engine/purge.rs`, `src/storage/purge.rs` | Existing purge protocol supplies exact invocation observations and guarded named begin; no second scheduler or durable progress registry |
| `src/storage/metadata/purge.rs`, `src/storage/metadata/purge_budget.rs` | Same-TX fresh admission with bounded raw resolution and conservative exact-representation witness counting; original Context replay first |
| `src/storage/error.rs`, `src/storage/metadata.rs` | Typed pre-work size refusal and private regression fault/pause seams/module registration |
| `src/storage/native_purge_tests.rs`, `src/storage/native_purge_tests/api1_consumer.rs` | Actual transport, failure/race/replay, bound, cancellation and unchanged-source regressions |

## Bounded argument and executed acceptance

See [estimator](../../implementation/native-api1-purge-frame-estimator.md),
[approved plan](../../implementation/native-api1-purge-frame-admission-plan.md),
and [pinned writer audit](../../implementation/native-api1-purge-bound-source-audit.md).
The bounded path shares one100,000-examination/256MiB-source counter from raw name
resolution through the union scan; each value is capped before decode at
min(receipt budget,8MiB). A capped/partial/malformed-late scan cannot authorize work.
Transient Value allocation is per bounded component, not a whole proof union.
Existing redb pager memory is not claimed to fall under this added source-byte counter.
These conservative fresh-transport limits can refuse valid high-degree/large-unrelated
stores. They are not new canonical schema, export or migration limits.

The receipt counter uses exact to_value representation, matching project() and f32
widening. It counts all target deletions/history, incident edges, and before+after
source peers per edge (duplicates intentionally overcount), including targets first
changed as peers and deleted later. Mutable edge_count/recordGeneration and all
receipt counters use maximum typed widths. Actual compact receipt N has no literal
control bytes; actual outer E with all longest fields and512NUL diagnostic bytes
therefore bounds the escaped frame by E+2N. No arbitrary padding or universal fixture
maximum is claimed. The success response has its own maximal serializer check.

| Obligation | Actual evidence | Limit of claim |
| --- | --- | --- |
| Exact serialization bound |010ValueN/E+2N, inverseN/N-1, f32 exact counter/overflow; start/child/final, I/O/control/recovery/nested committed, allocated/unknown worker, final-only metrics, NUL/quote/UTF8 | Actual typed variants plus serializer inequality; fixture sizes are not universal maxima |
| Old/new real transport |006 actual postcommit frame-loss RED;010 old1MiB refusal before meta/edge bytes/G/IDs change, then8MiB complete matching child | No new local MCP cap or protocol |
| High peer/history |0101target/400peers/32history: full1,302,161B frame;1000rows:1,572,891B | Both exceed old cap and preserve complete evidence after negotiation |
| Profiles and bounded lookup |010 real migration with large escaped target/unrelated descriptors; both peer capacities refuse unchangedmeta/G/IDs; exact-before-fold and malformed/oversized-late/cumulative cap unit probes | Same-TX shared Scan; no early success or full namespace Vec |
| Ordinary exclusion |010 actually captured native graph + fresh catalog + second purge reject;011 adds captured numeric policy, valid native batch reservation and physical compaction | Other native record/lifecycle families source-audited through native guard/final shared TX; legacyaccess negative is not feedback/exposure execution |
| Permitted continuation |010 competing same-parent one-row chunks; target-to-target and foreign incoming edges; peer then deletion exactoriginalrecordG.next; every child fits admitted budget | No other active purge or new source/history permitted; no fixed chunk-size assumption |
| Historical oversized evidence |010 real oversized private keyed start, oldName rejects active operation without invented evidence, Context replay unchanged before fresh validation, complete absent name and recreated freshID | Public Name is not a historical resume API; no receipt rewrite/substitution |
| Unknown/cancellation |010 actual lostbegin/lostcompletion/lostchild ack frames, fullknownstart or no receipt as appropriate, actualallocatedID only, unresolved work, close/reopen; direct+bounded cancellation retains worker locks | No known final/readiness upgrade after worker loss; no guessed identity |
| Physical observations |010 actual orphanFTS count, directory present/absent, empty cleanup and readiness; competing completion returns final MetricsUnavailable | No guessed zero count or automatic name retry |
| Foreign pending precedence |Existing focused actual foreign-first case preserves known matching start; matched child stays child even if later Complete | No error-path readback to upgrade scope |
| Compatibility/parser |Focused real MCP eras/remote/oldframe paths and malformed typed numeric/extra fields | Full default-all-targets suite passed; no broader joined-tree claim |

## Immutable run chronology

| Run/source | Result | Interpretation |
| --- | --- | --- |
| before001423b |0PASS/1FAIL | Actual nativeComplete retains receipt in plain string but loses structured/typed evidence |
|00267a |1PASS | First actual consumer projection |
|00307d and004b7 |Each4PASS/2FAIL | Value numeric text equality rejected valid retained f32 bits |
|005c896 |11PASS5.81s | Typed numeric decoding plus exact field shape corrected |
|0064853 |11PASS/1FAIL33.62s | Actual1000-row fullreceipt exceeds1MiB and daemon fallback loses evidence |
|007718 |13PASS38.70s | Superseded proof claims: typed-vs-Value N and pre-cap namespace decode; no unsafe-admission witness claimed |
|008369 |15PASS38.90s | Corrected runtime and exact counter/raw lookup tests |
|009a097 |20PASS/1FAIL40.35s | New test observation through intentionally poisoned storage; preserve run, no runtime correction |
|0108c990 |22PASS40.64s | Corrected observation plus all typed frame inequality and bounded cancellation |
|01190938 |22PASS40.70s | Added actual policy/batch/compaction exclusion assertions; no production changes |

010 measured E3868, N/start861/child9452/final377; largest frames4663/13809/4136
versus bounds5590/22772/4622. Unknown without receipt236B. memory.max0/OOM0.
009 memory.max322/OOM0;007/008 reclaim events disclosed in respective manifests.
All runs Rust1.94.1, offlineDocker2CPU/4GiB/jobs2/testthreads2, read-only source and
shared registry, single private package-cleaned target. No personal data/daemon,
credentials, network, pilot seeds or sockets are mounted. Source/log/executable
hashes are recorded per run. Frozen full011 passed1378library/3CLI/12examples (1historical generator ignored),
602.33s library; full command640s. Formatting and all-feature/all-target Clippy
passed separately with warnings. This is not all-feature test execution.
Full cgroup peak3,937,464,320B, memory.max0/OOM0; checks max0/OOM0. See
[candidate011-full](candidate011-full/README.md) and
[candidate011-checks](candidate011-checks/README.md). Source90938aa5 is identical
across focused/full/checks; runtime369 remains unchanged. The executed library
SHA is a1a7424c34c1ac28b6dcdaccfbd6f998eb76495b40754c8dbd6ec30ef7329a7c.

## Retention and remaining work

Protected active root `/tmp/recalld-native-api1-purge-validation`, continuing sole
target `/tmp/recalld-owned-native-catalog-ids-full-xk182ybj/target` and shared registry
`/tmp/recalld-unit-tests-l__ja1yo/cargo/registry`. No cleanup/recovered-byte claim for
this unpublished leaf. Release completed raw through central cleanup after PR/results,
retaining code/reports/worktree and only explicitly needed future inputs.

Actual HOME focused and full entries are verified in the existing experiment;
[focused manifest](focused-final-home-entry.json) and [full manifest](full-home-entry.json)
record exact entry hashes. No raw archives were copied.

Before publication: exact final source/report review, then explicit publication approval and exact-head CI/frozen handoff. All local
focused/full/fmt/Clippy checks above are complete; no runtime changes followed. Ordinary native startup and public2 remain separate
integration gates; this native API1 purge consumer does not complete them.
