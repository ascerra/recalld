# A6 combined controlled group/context consumer

**Bounded actual consumer PASS. Full A6 remains incomplete.** This new named track composes the existing public actor, complete external RPC accounting, source-only groups, raw/packed scoring, actual pinned BPE context and same-core evaluator controls. It introduces no runtime Rust, engine, tokenizer, clock, registry or scoring implementation.

Direct base: **32ab3b3f754e778403a53d400b36f249a3f9fd2c** (main190). Validated implementation source: **6f8e4713c16004d0fee6c428ea5171288cc3f3b0**. Later report-only seal commits are not the executed source. Required upstream consumers #184/#187/#188/#190 are already in this base; no convenience parents. Halley released these runner/actor hunks after Curie/Huygens confirmed no overlap.

## Actual contract

Track `serialized-mcp-combined-control-synthetic-diagnostic-v1` keeps manifestVersion 1/sourceMappingVersion 1 and explicitly uses **traceSchemaVersion 2**. Legacy manifests/readers remain version 1 and reject unsupported versions. The new profile pins the existing legacy-checkpoint-sweep constructor, private controlVersion 1, lifecycle-owned unadvanced clock and canonical allocation; general RNG/ID/reset guarantees remain unclaimed. Only inspect/barrier and deliberately rejected negative controls are consumed. Snapshot/advance/sweep capability presence is not an executed guarantee in this slice.

The actual owned evaluator uses production MCP parsing/handlers on the same core as the private controls. The actor receives only its current projection and public stdio; both actual control descriptors are denied on every invocation. A post-delivery barrier is **not a response ReadView**: query generation/candidate diagnostics remain unavailable. The actual inspect runtime-state shape and exact advertised capability object are checked, not accepted as a permissive subset.

Trace2 separates RPC attempts/outcomes/errors, rendered context observations and evaluator-only barriers. The reconciler binds every row to expected run/environment/episode/group identity from the validated manifest; context must follow a matching successful recall. Attempts are appended/flushed before consuming outcomes, not necessarily before remote mutation, and no power-loss durability is claimed. Context adds no RPC/provider call. The existing independent context verifier reconstructs the complete rendered/tokenized artifact from actual public payload, not self-reported IDs or hashes.

## Source/build/runtime identity

- Frozen 531-source-file map SHA256: **442074a4e817f1deac9cdd180a889dda6103a606b6617fefa73d7c8cfaac9a3c**; [map](evidence/source-files.json), [build input](evidence/build.json).
- Local image: `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
- Actual CLI SHA256: `80b94d856e97f94f5c6d7106b5ee58c1d1a00d2cf678b9eae0814ffb8261a393`.
- Actual owned evaluator SHA256: `5f1f627715ed95cee714f992c4111ea5265e7fe8219d304aa230c0ed91e4e93a`.
- Actual scorer SHA256: `f0bd69e091f7486412a041f0487b5d7ca25ffc73b8e6314eae08c07a009a984b`.
- Source hashes and all three executed binary hashes were unchanged before/after runtime. The Rust/Cargo source is unchanged by this PR; source/compiled-scorer provenance was refreshed after each Python correction.
- Offline 2 CPU/4 GiB, one reused private target, shared retained registry read-only, no cache clone or personal paths/config/credentials/socket. Source, binaries and tokenizer bundle were read-only during actual execution. No network/paid model/dataset acquisition.
- Required binaries only: `cargo build --features bench --locked --offline -j1 --bin recalld --example owned_evaluator --example score_mcp_episode`. **No full Rust library suite was run or claimed.** The current required-target build took 39.679s; peak 4,294,967,296 bytes, OOM 0. The initial successful build took 91.120s; peak also reached 4 GiB with cgroup `max` pressure events, OOM 0. This is correctness validation, not a performance result. Existing library warnings are retained, not claimed lint-clean.

## Actual acceptance

All three seven-member cohort executions passed on exact 6f8 source. Each has 24 eligible default records plus a foreign decoy per episode, four successful questions and a separately expected offline replay miss. Every successful query returned exactly 20; the decoy was excluded. There are five source/history groups; exact duplicate histories and the declared shared-history pair stay together.

| Actual cohort | Episodes / queries | Scored / zero-label / unknown | Actor RPCs / provider requests | Contexts / strict barriers | Full rendered tokens / omitted entries |
|---|---:|---:|---:|---:|---|
| default 4096 | 7 / 28 | 9 / 6 / 13 | 252 / 210 | 28 / 49 | 1077–1079 / 0 |
| boundary 128 | 7 / 28 | 9 / 6 / 13 | 252 / 210 | 28 / 49 | 26–114 /18–20 of 20 |
| boundary 128 repeat | 7 / 28 | 9 / 6 / 13 | 252 / 210 | 28 / 49 | 26–114 /18–20 of 20 |

Thus the principal proof is **21 episodes / 84 successful questions / 756 actor RPCs / 630 provider requests / 147 barriers**, not 21 independent real-data groups or a quality study. Raw and packed metrics use the same complete roster, labels, mappings and denominators in each split.9 + 6 + 13 = 28 is separately checked against the fixture; unknowns and known zero labels never become scored successes. No paired confidence interval is needed or claimed for this composition test. [Semantic audit](evidence/semantic-audit.json) and per-cohort grouped outputs preserve the actual scores.

The seven barriers per episode include initial admission, ingestion, each current question and the expected replay miss. Each requires the same store lineage, committed decimal generation, pending 0 and exactly graph/vectors/FTS/entities ready at that generation. False queries/miss preserve observed generation and captured runtime state. This does not claim an independent complete canonical-byte oracle, response ReadView, reset or all-writer proof.

Per cohort, private control-wire accounting separately records 196 attempted control calls and 42 expected error ACKs (unsupported reset/logicalHash, invalid deadlines and foreign-store rejection). These are **not added to actor RPC counts** or treated as provider calls. Actor buffers/provider bodies are measured; HTTP framing, backend retries and model usage remain unknown. Private control counts are observed calls, not measured control transport byte totals. No nested duration sums or per-RPC provider causality are inferred.

The actual run completed in **82.897s**, peak **1003008000bytes**, all memory pressure/OOM counters0, driver/container retired. Shared-host timing is diagnostic only. [Runtime receipt](evidence/actual-result.json), [command](evidence/actual-command.json), [before](evidence/executables-before.sha256)/[after](evidence/executables-after.sha256) hashes.

## Negatives, canaries and compatibility

- Entire cohort preflight precedes output/provider/store. Actual corrupted last member, trace1 on the new track, old profile with trace2 and wrong executable hash all reject without runtime output.
- The frozen negative cohort omits one middle member's current-query cache entry. Its actual provider returns 400 and MCP returns an error. Full roster is **3 passed / 1 failed / 3 unattempted**; no raw or packed cohort aggregate exists. No failures become empty pages or a paired intersection. Earlier successful member artifacts remain retained.
- Actual gold-label perturbation leaves current actor projections, public requests and provider inputs/vectors unchanged. Environment/cache keys change. Actual future-question perturbation leaves earlier deliveries unchanged. In this observed run gold context renders/tokens/prefixes were identical; future change altered one render/token count and no source prefix. Raw UUIDs/renders remain intact: no source-bijection or token normalization was used, and no universal inclusion/startup determinism is claimed.
- Current source **legacy accounting and context tracks pass**. Historical OWNED profile rejects the newer constructor's exact capability object before actor work, as expected; it is **not reported as positive compatibility**. The historical OWNED helper, fixture and profile remain unchanged. The combined track supplies the positive current-constructor/control consumer.
- Twelve focused new checker tests pass. Existing public 16/group 23/actual-tokenizer 9 checks passed on 49a; those unchanged components' evidence is retained separately. The new checker was also replayed over an actual successful retained trace: valid input passed, all 8 foreign-identity/failed-query/non-query/correlation mutations rejected. [Proof](evidence/retained-trace-checks.json).
- Pending/stale-view rejection and a simulated control timeout are explicitly checker tests. Actual invalid deadline/foreign-store control errors are exercised. **No real delayed-core-worker timeout is claimed** and no new core fault hook was added.

## Preserved failures and attribution

1. Two host compatibility commands stopped at argument parsing; no test executed. Correct asset/episode invocations later ran in Docker.
2. Export002 rejected two integrated schema test assets absent from the inherited allowlist. Exact file pins were added; no wildcard.
3.003 stopped before compilation because the retained private Cargo registry contained onlyCACHEDIR.TAG. The explicit retained shared registry contained every locked package and was mounted read-only.004 stopped at target admission because the command omitted the existing `bench` feature. Both are build-driver failures, not runtime failures.
4.006 executed source 49a and failed at the first control handshake: current owned evaluator had checkpoint/clock/sweep capabilities, while the old helper expected only inspect/barrier. **0 successful episodes;1 failed / 6 unattempted; no actor/provider work or scores.** Exact wire/roster retained. C-only adaptation uses the existing current capability/inspect helpers; no runtime weakening or old-profile rewrite.
5. Integrator source review found that trace2 reconciliation lacked validated identity binding. The actual checker accepted four foreign-identity mutations; RED log retained. Corrected 6f8 requires the manifest-derived identity plus successful query correlation. This is a consumer-checker defect, not a production runtime failure. An intermediate new test used invalid empty MCP content and failed in existing rank projection; that test-fixture failure is retained separately.
6.007 succeeded but its prepared 008 runtime stage was **never launched** after the checker finding.009 rebuilt exact 6f8 provenance;010 is the successful full actual run. No failures or runs are relabeled as a clean first attempt.

Compact failure logs, exact failure wire/roster and successful receipts are under[evidence](evidence). All raw attempts remain protected at `/private/tmp/recalld-a6-combined-control-validation`; failed 006 owned/scorer binaries were preserved by APFS clone, not a whole target archive.

## Scope and next dependencies

This closes a bounded part of residual C. Native writer/keyed retry/lifecycle supervisor A, broader reproducibility B, upstream conversational golden adapters D, recorded extraction/answer/judge caches and transformed provenance E, and chronological budget/reward consumer F remain separate code/proof gaps. General native admission/public2/reset coverage is not established here. Existing prepared evidence is integrated but this legacy route cannot invent its response provenance.

Human/rights/real heldout and promotion gates remain separate. The recorded static SciFact 300 result remains no demonstrated gain, with descriptive resource regressions and no promotion; this synthetic consumer does not amend it. No new empirical study, paid call or training was performed.

Cleanup: **0 raw paths released/deleted for this unpublished slice**. Protect the current raw root, failed-source binaries, active worktree/Git, sole owned-native-controls cargo/target, shared unit-tests registry, existing tokenizer bundle and all empirical inputs. After publication/results, successful disposable roots may be released centrally; explicit retained failures/future inputs remain protected. Existing central cleanup ledger/protections are unchanged.
