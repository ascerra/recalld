# Ordinary native startup and consumer activation plan

Source audit and ownership handshake, 2026-11-14. This document replaces the
older preliminary inventory, whose provenance remains in git history. Planning
branch starts at PR121 `3fa91d4e542ec8ed5519fe4c3b87e9e665043a40`;
its validated runtime is `75bc809d048c4762ef56ae36d08d2395b482870b`
(1093 library + 2 CLI + 5 focused executions, fmt, Clippy with warnings).
No startup gate is changed by this document. Individual gated PRs are intermediate;
the assigned outcome includes the real final consumer integration.

Feynman owns existing engine/system/daemon admission, strict canonical history,
offline/recovery composition and the readiness join. Huygens owns public routing,
API1 compatibility, discovery and capability activation. Pascal owns lifecycle
wire and native exposure/maintenance semantics. Curie owns producer roots; Hume
owns graph proofs and A2 selection; Halley owns accepted work, clocks/IDs and the
publication registry. Shared files are coordinated before edits.

## Exact prerequisites and actual ancestry

The adjacent `native-startup-dependency-closure.json` records full SHA values and
pairwise `git merge-base --is-ancestor` results. These are immutable local git
facts at this audit, not combined-runtime evidence. Owner-reported validation is
separate from our PR121 runtime proof. Moving branch names are not freeze inputs.

| Required surface | Frozen ref | Why needed / subsumed dependencies |
| --- | --- | --- |
| Replay consumers and native purge current reader |121 `3fa91d4` (runtime75bc809) | Contains105,108,110,114,111,96,82 and validated retired-binding authority. Current-reader accepts only known1/2 and exact original-lineage anchors. |
| Catalog/numeric policy wire |103 `1b4876d` | Actual93+94+96; creation defaults, keyed no-change receipts, canonical publication, old receipt identity. Not present merely because121 has core policy types. |
| Native graph proof/no-change |97 `292fb29`;108 `b5673ab` | Both already ancestors of121. Do not add redundant merges; actual standalone producers remain separate. |
| Native single insertion with explicit/configured links |118 `f910c00` | Actual typed producer; already in profile and owned-native-ID leaves. Links belong to original store commit, not an optional later fire-and-forget call. |
| Profile-ID selector and retained request reconstruction |`d7c4d5fa3d39e14f3bc2dc4be5554e2e1bf1c8c8`, runtime1d9e7d1 | Owner reports1080+2/14focused/fmt/Clippy. Direct118 child. Full public wire integration and actual retired-purge/fork composition separate. |
| Batch wire/CAS/budget |`abd03717b0f0dfda2ac8906445abb560aba8b716`, runtimeccae79b | Owner reports1095+2/6wire+7CAS/fmt/Clippy. Actual106+88; does NOT contain96. Huygens joins only actual profile/118/96 for single+batch publication and profile dispatch. |
| Native scheduled/manual maintenance |107 `785dfec` | Native dispatch in `DecaySweepRunner::execute_sweep_with_work`; not inherited by121. Preserves sampled clock, final record/policy/graph recheck and unknown receipts. |
| Native query exposure |119 `8ab79b4` | Contains113 `149beb6` coherent get/similar and116 exposure core; requires actual owner publication and query policy. |
| Metadata/catalog pages and duplicate scan |112 `5951f75` | Contains100 `ff57780` and88 `3961f66`; no need for three merges. API1 read compatibility and public2 activation still required. |
| Honest readiness observation |`cc865c296247a55cadb09081e349c059c8724619` | Direct96; must compose its actual counts/unknown stamps with121 native2 checks, not replace them. Owner reports1002+2. |
| Native owned allocation |`a5865da02969631a5031f65c6e43269701d7a254`, runtimeb1ebb689 | Actual104+118; owner reports1086+2 and focused checks. Not a native genesis/reset or public activation certificate. |
| FTS rebuild correction |117 `bf7c2484018297b7f9af30e98d3214581f33f351` | Required actual index content correctness, independent of matching publication G. Halley reports consumed in120 replay evidence. |
| Native namespace clock/bootstrap |Halley child535605, validation pending at audit | Not a frozen implementation parent yet. Use owner clock; never a second wall clock or provider-derived observation time. |
| A2 coherent semantics2/current-history |Hume follow-up not frozen at audit | Historical semantics code alone does not make113 V1 similarity/coherent selection correct. Await actual integrated selection/budget/filter evidence. |
| Feedback/forget public wire |Pascal child of121+88, validation pending | Actual adapters consume typed bindings/terminal identity and preserve full known receipts. Do not add119/batch as convenience parents. |

The current graph's maximal leaves among the already-frozen inputs are121,103,
profile, batch-wire,107,119,112,observer,owned-native-IDs,117. This is a **coverage
calculation, not a request to merge ten parents**. Huygens's real insertion/batch
consumer will subsume profile+batch-wire+96; Pascal's lifecycle wire will subsume
121+88. Recompute closure after their freeze and after A2/clock handoffs. Choose
one actual consumer as primary base and merge only missing required leaves. No
broad validation checkpoint may become a shipping parent merely for convenience.

## Actual consumer and missing-adapter inventory

Source anchors below identify which frozen tree was inspected. Where we have
only an owner handoff, that qualification is explicit. No new general content
update route is invented: existing insertion/supersedes, lifecycle changes,
tombstone, destructive maintenance, graph and catalog routes are the inventory.

| Surface / real ingress | Actual authority and observed gap | Required closure / owner |
| --- | --- | --- |
| Engine open / embedded callers |121 `storage/engine.rs::open` calls `open_mode(false)`; `open_native` is private. Both share the same storage type and recovery sequence. | Feynman: choose admitted mode from strictly inspected canonical format under ownership; no second engine or bool selected from API version. |
| System and daemon startup |121 `system.rs::with_clock` opens ordinary engine, constructs views/QueryEngine, creates missing default using schemaNone, rebuilds then starts sweeps. | Feynman: real native bootstrap uses canonical catalog writer/default policy and owned clock/IDs; publish all views before workers/listeners. Huygens owns actual route capabilities. |
| Single store MCP/REST/RPC |Profile d7c `mcp/bridge_adapters.rs` calls legacy `insert_reserved_with_work`; REST `api/adapters.rs:455` same; daemon dispatch forwards old store port. | Huygens: actual native single producer from118/profile with original input/default/hash semantics, stable-ID replay before provider/current lookup, current-G completion and original receipt. |
| Ordered store/forget batches |Batch wire abd has private native consumer, full parent CAS and bounded replies; API1 still needs mode-aware dispatch. | Huygens + Curie: retain ordered manifest/child identity, all indexed outcomes, no atomic-batch claim; actual96 completion per child/bookkeeping. Pascal supplies forget core. |
| Explicit parent/supersedes and configured insertion links |118 native producer commits retained links with insert. Calling legacy follow-on autolink would duplicate work or break crash durability. | Huygens/Curie: use original producer plan; reject unsupported options before provider/allocation until participating route exists. Never silently skip enabled links. |
| Standalone graph/autolink/entity/temporal writes |Profile d7c `graph/autolink.rs::publish_edge_plan_with_work` still calls legacy `put_edges_with_effects_with_work`; `explicit_links.rs` calls that same function. NativeGraph coordinator exists separately. | Curie producer integration with Hume97/108 proofs; actual native dispatch, final peer tokens, same-namespace/cycle checks, keyed no-change receipt, worker-held lease and accepted descendants. This is an explicit missing consumer, not closed by tracker coverage. |
| Feedback/reinforce and forget single |121 actual native coordinators use same-guard retired binding/currentG; old transport adapters still use legacy helpers. | Pascal wire plus Huygens API1 dispatch. Scope by immutable record/terminal UUID, optional name assertion, replay before fresh clock/CAS/policy. Missing proof retains known receipt. |
| Access recording after recall/get/fetch/similar |119 `search/adapters.rs` dispatches `record_native_exposures_with_work` from owned mode with completion/query policy. | Join119 actual reader/observer. Both API defaults true; UUID dedup and ordered per-namespace partial receipts; separate pre-exposure readG from receiptG. False mode writes nothing or defers nothing attributable. |
| Namespace create / policy / retention routes |103 actual typed catalog/numeric policy consumer exists; API1 create still builds schemaNone config. Legacy retention entry points also require audit on final tree. | Huygens public adapter; Feynman system default bootstrap. Preserve fixed expanded defaults, dynamic optional selector identity, legacy_access_v1, keyed no-change G once without revision churn. |
| Delete namespace / force purge |103 `mcp/bridge_adapters.rs:2527` still calls legacy `purge_namespace_checked`.105/121 actual native completion exists. | Huygens public force/name/error projection; Feynman existing canonical owner and readiness. Resolve stable ID once, block fresh inserts while Purging, retain child failure receipt independently from Complete canonical progress. |
| Background/manual decay and irreversible loss |107 `decay/sweep.rs` selects native pass by owned schema; original clock shared across pass, all IDs captured before transitions; native phase/hard-delete use typed witness/ledger. | Join107 scheduling/completion with121; final guard still graph then storage, count stale skips separately from exact no-op, retain Ghost vectors where current patch does. No persistent RIF/new default. |
| Compaction / derived phase checkpoints / shutdown |121 generation compaction uses legacy mutation guard.107 native sweep explicitly avoids legacy compaction and side-channel bitmap persistence. System shutdown calls phase persistence. | Feynman must distinguish supported canonical compaction from derived bitmap rebuild/persist, audit native guard and public/manual exposure. Any remaining unsupported compaction is explicit, not silently called by native sweep. No unreported G advance or stale all-ready stamps. |
| CLI/importer single ingestion and LoCoMo |Profile d7c CLI uses `/v1/memories`; raw importer uses same client. `bench/locomo.rs:1284` directly uses legacy InsertMemory. | Huygens wire covers real CLI/importer; Curie typed benchmark producer if native fixture is selected. No public fixture escape, warm-only accepted labels or copied in-memory entity repair. |
| Recall/search whole pipeline |96 actual publication capture;113/119 coherent payload and observer; A2 follow-up remains. | Hume actual eligibility before caps, current/history terminal resolution, bounded graph/version work, UUID ties and coverage; Huygens selector/envelope. Cannot call effects_epoch or canonical-only capture a whole-query ReadView. |
| By-ID get / batch fetch / cross-namespace similar |113 actual get/similar selects V1;119 native observer. Namespace/profile exclusions must match captured view and returned payloads. | Hume A2 + Huygens wire; point reads fail closed if unavailable, no false empty. Preserve explicit allCompatible scope and per-result namespace. |
| Metadata lists / tags / stats / namespace pages |100/112 same-transaction canonical pages/cursors, structured labels;88 exact duplicate snapshot is bounded private backend with real consumer. | Huygens public adapters; non-observing reads need no provider/automatic attestation. Exact duplicate scan is not general query coherence. Native/API1 legacy payload checks on final tree. |
| Barrier / snapshot / admission close / cancellation |96 registry actualG; observer cc865 actual counts;121 strict native2 purge reader. | Feynman+Halley compose exact same reader, root/descendants and all required publications; no sync lock across await, worker retains lease through cancellation, unknown remains unresolved. Snapshot also stops admission and reaches zero accepted work. |
| Offline schema inspection / logical export/import / backup / production restore |121 contains strict mixed history and completed-purge export/import/fork proof; newer batch/profile/history tables must survive final composition. | Feynman strict inventory + Pascal restore lineage: source immutable, bounded validation, fresh private destination/staged publication, exact receipts/ledger/labels/provenance/f32bits, no resurrection/unreachable log export. |
| Private recovery / interrupted native batch or purge |121 permits exact native purge recovery; batch abd permits exact validated native batch2/3 recovery. Each branch alone lacks the other's new allowance. | Feynman join both narrow native-only recovery predicates and all canonical validators. Offline inventory continues rejecting unfinished work. Reopen never manufactures missing client batch payload; matching resubmission owns it. |
| Raw Rust writers / offline fixture loaders / repair |Raw restrictions remain on ordinary legacy methods; replay metadata helper and offline snapshot do not activate writers. | Feynman final callsite audit and compile benches/examples; no production public benchmark escape. Native unknown schema/history always rejected before source mutation. |

Curie's immediate assignment remains the isolated profile+121 real purge/name
reuse/fork proof; the standalone graph row is a subsequent ownership plan, not a
request to disrupt that validation. The final active producer list must enumerate
all callsites, including shutdown/admin and transport fallbacks, before admission.

## API, disk and lifecycle compatibility matrix

| Disk / request | Required selection and compatibility |
| --- | --- |
| Existing legacy disk + omitted/explicit API1 | Preserve payload/default/error behavior and legacy lifecycle. Existing canonical receipt/effects containment still applies. |
| Native disk + omitted/explicit API1 | Supported legacy tool/HTTP/RPC operations select native participating owners by disk mode. Preserve legacy outward shape and configured RIF behavior; recordAccess remains true. No accidental legacy-only writer failure, implicit migration or fabricated provenance. |
| Native disk + explicit API2 | Actual native public adapters, strict selectors/envelopes/receipt/errors, semantics2 and capability truth; explicit lifecycle migration remains separate. |
| Legacy disk + explicit API2 | Huygens validates support per actual operation/capability. No implicit disk upgrade. Unsupported writers reject before provider/allocation; metadata operations only advertise actually implemented behavior. |
| Any disk + unknown API/schema/required proof version | Strict rejection at correct boundary. A schema error must not silently select API1 or initialize absent tables. |

New and migrated namespace lifecycle stays `legacy_access_v1`. Unknown embedding
provenance remains unknown; provider configuration identity is not proof of old
weights. True exposure under either lifecycle uses its policy; false reads cannot
record receipts, alter graph/lifecycle or queue later behavioral work. API2 does
not opt into persistent RIF, approximate search or a different canonical backend.

## Source-confirmed API/context coupling, pending coordinated review

Native guards in `metadata/native_catalog.rs`, `native_policy.rs`,
`metadata/native_graph.rs`, `metadata/native_store.rs`,
`metadata/native_records.rs`, `metadata/native_lifecycle.rs` and
`metadata/native_maintenance.rs` require context contract2. Purge additionally
uses `ctx.contract_version` as its disk/progress selector. A wire adapter cannot
fix this by labeling an API1 request API2: that loses original request identity.
The detailed per-writer plan is [native-request-context-plan.md](native-request-context-plan.md).
Core edits are held for coordinating-agent review. This adds a concrete activation
prerequisite, not an alternative engine or a reason to leave API1 permanently gated.

## Concrete constructor, recovery and readiness sequence

1. Reuse `StoreOwnership::acquire_recovering` and directory lock. Inspect existing
   control/schema on disposable copies while both locks remain held; mode choice
   and full validation occur inside this ownership lifetime, not an unguarded
   preliminary open. Missing/ambiguous files and historical-read-only roots fail.
2. Dispatch the existing strict preflight by validated disk mode. Native-only
   repaired-copy inspection may validate exact pending native effects/purge/batch;
   ordinary offline inspection still aborts repair/rejects pending work. Validate
   all required tables/proofs, ancestry, text gate/descriptor/extents, vector
   ownership/hash, namespace/profile/policy and edge reverse history before
   writable metadata open. Preserve old immutable receipts and source bytes on
   every malformed-after-repair negative.
3. Open the same metadata/edge/text/vector engine. Use raw recovery reads solely
   for preconstruction enumeration, apply exact canonical pending effects, resume
   allowed purge work, then startup validation/orphan cleanup. No general guard
   weakening. Batch resumption still needs original payload and stable child IDs.
4. Construct existing graph/vector/FTS/entity/cache/query components. Create a
   missing default namespace through the actual catalog owner with owned clock/ID
   semantics (never schemaNone in native mode), before final publication. Failure
   retains canonical receipt; do not present rollback or provider attestation.
5. Rebuild/validate actual component contents, then publish the same actual
   GenerationRef through the sole registry. Catalog/policy/no-change receipt G,
   exposure G and batch bookkeeping invalidate it even without effects_epoch
   change. Compare all canonical scope/policy/record and derived publications.
   Bound stale retries; exhausted/unknown capture is unavailable, not empty.
6. Only after the actual reader/writer adapter closure passes, admit background
   sweeps and transport listeners. Daemon readiness/health/capabilities reflect
   owned state. Shutdown closes admission, awaits accepted transitive work, then
   checkpoints only appropriate derived state; cancellation cannot release a
   lease while its blocking worker still runs. Use graph -> storage order.

This is a change to the existing constructor and consumers, not a NativeSession
server, second registry, general allowlist or speculative engine replacement.

## Minimal implementation order and required proof

1. Freeze owner consumer leaves, recompute ancestry, and record explicit seam
   conflicts. Huygens single/batch/profile/96 join and Pascal lifecycle-wire/121
   join are actual shipping prerequisites. Curie's independent profile+121 proof
   does not become a broad shipping parent. Hume A2 and Halley clock must supply
   actual frozen runtimes; no invented completion from older component counts.
2. Feynman first joins only necessary readiness/recovery consumer leaves to
   exercise native batch+purge histories with exact4-view publication. Preserve
   cc865 unknown counts plus75bc version/schema/receipt checks and both pending
   recovery rules. Any required production glue ships with that minimal actual
   consumer dependency; validation-only glue cannot be discarded.
3. Close owned system catalog bootstrap, automatic/manual maintenance dispatch,
   shutdown and remaining producer adapters. Preserve real CLI/benchmark paths.
   Feynman implements admission only on the exact validated closure; Huygens
   changes public selectors/capabilities on that same source composition.
4. Validate focused failures first, then full fmt/Clippy/test/bench-example compile
   on the frozen composed source in offline Rust1.94.1,2CPU/4GiB/private target.
   Do not reuse a test executable after switching mounted source roots: separate
   source target or package-clean own target. Preserve all failed source/binary/
   log hashes; no full repeat merely for a documentation seal.

Final tests must use ordinary engine/system/daemon/transport entry points, not
only `open_native` fixtures: legacy/native x API1/API2 defaults; fresh and reopened
native owner; every actual writer followed by get/exact/FTS/entity/graph views;
policy/catalog-only and no-change G; observed versus false reads; same-key replay
before stale CAS after purge/name reuse and restore; partial/unknown batch/purge;
publication failure/cancellation; process exits before/after canonical commit;
malformed repaired-copy originals unchanged with ownership held; native mixed
history through logical export/import/backup/fork and original-lineage rejection.
FTS ranking/content parity, A2 selection and performance/release evidence stay
separate from registry stamp equality. All required component and final consumer
checks must pass; no individual gated PR establishes ordinary serving readiness.

## Coordination provenance

- Huygens committed source matrix at `abd03717b0f0dfda2ac8906445abb560aba8b716`,
  `docs/implementation/native-api-activation.md`; read directly from git here.
- Curie supplied exact profile d7c seal and source callsites on1d9, including CLI,
  LoCoMo and standalone graph; our git source reads corroborated writer calls.
- Halley supplied a586 allocation, cc865 observation and117/120 FTS composition;
  clock535605 was explicitly not final. Counts above are owner-reported.
- Feynman directly read121 constructor/system/validation/compaction/shutdown,
 103 purge adapter,107 sweep dispatch,119 access recorder, profile graph writer,
  and batch abd recovery predicates. No runtime reproduction is claimed by this
  source-only plan or ancestry calculation. All work documents live in this repo.

Hume confirmed the historical semantics2 leaf `3b288980` and FTS eligibility
`f2a5ba51511d89aa82894e53d2a72c9ec937e320` are not the missing actual113/119
coherent semantics2 consumer. That remains his next frozen dependency. Graph97
and108 prove typed core transitions, not every producer/public route.
Huygens confirmed public purge force/name/error projection ownership; Feynman
keeps its canonical completion and system bootstrap. His next insertion join is
only published122 batch-wire abd037 plus validated123 profile d7c, not an
unrelated catalog or exposure ancestry. Pascal confirms current lifecycle wire
is private application2 only; API1/native core context compatibility is separate.
