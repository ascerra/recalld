# E1 recorded-answer consumer: actual synthetic provenance PASS

**The actual consumer passed; no answer-quality or full-A6 claim.** This named track consumes prerecorded synthetic quotations at the existing actual public-query/context boundary. It adds no model invocation, engine, production runtime, tokenizer, scorer algorithm, clock, native adapter or metrics registry. Recorded answer quality, model-token identity and production cost remain null.

Direct parent is published PR193 `362de51720859c5985a1042b702fe139dba34429`. Executed source is **`249ff40e8149743efcddcbe2dfcd100bb5171c4e`**, following implementation57d5bb59 and a test-only export-fixture correction. Later documentation seal commits are not the executed source. No convenience parent or changes to frozen193.

## Actual boundary and versions

The existing serialized MCP actor receives only the current query. Its rendered artifact is independently reconstructed from the actual public payload using the pinned tokenizer. After the actual current query ACK and same-core barrier, the parent looks up an exact recorded ANSWER input/configuration key and invokes the existing actor's local answer phase with one current envelope. The actor consumes the exact output bytes and emits the parsed answer; the parent validates those bytes and citations against the actual packed public payload. Both public and both private-control descriptors were closed in all51 actual answer-actor invocations. The same MCP child/session remains owned by the supervisor.

The actor receives neither the complete role cache nor future questions/evaluator labels. Actor-safe parser code imports no fixture/schedule factory. The answer phase performs no additional RPC/provider request. Exact quote membership proves provenance, not proposition support or that a quotation answers the synthetic question.

Track `serialized-mcp-recorded-answer-synthetic-diagnostic-v1` has manifestVersion1/sourceMappingVersion1/traceSchemaVersion2 and an independently declared **roleTraceSchemaVersion1**. RPC/context/barrier trace2 is unchanged. Separate role traces link run/environment/episode/group/query/delivery/public step, actual context, role configuration, key, input/output and cache artifact hashes. Complete results bind both stream digests. Old C consumers reject E before effects; no unknown kind is silently added to trace2.

The key covers exact current question/rendered context, prompt/tokenizer/context settings, role/parser versions and declared behavior-affecting runtime/embedding/clock/allocation/RIF/graph/cache settings. Full environment/source/cache-artifact identities are separately pinned, avoiding self-reference. Evaluator labels and future schedule are explicitly outside this role-config domain; no actual actor input is normalized. Changed prompt/config/runtime or actual render cannot silently rekey an old output. The internal profile explicitly declares `modelRevision=null` and synthetic-recorded origin, not a fictional model revision.

Checked-in fixture SHA256 **`7886bd8908b1b59d6e02aa47be66e7aec9ffba15b5aae3388e0bb830b19efe64`** freezes28 retained actual193 contexts and synthetic quotation outputs before E execution. The antecedent input receipt SHA is `a4216b7cbab8114ec0c42d9c0ff3af5727727272f64337ac9fb6001d15695f4f`. Actual first-cohort render/key equality passed without UUID/token/source-ID normalization or regenerating outputs.

## Frozen source, binaries and resources

- 536 exported source/input files, map SHA256 **`cec33878b881c46a90aae1c60bb4f9f1801ec70be234b49687c44a98d263b639`**. All file hashes were verified again after actual execution.
- Actual inherited owned evaluator: **`5f1f627715ed95cee714f992c4111ea5265e7fe8219d304aa230c0ed91e4e93a`**, unchanged from193. Rust/Cargo sources are unchanged; this is reuse of that exact runtime, not a newly measured core implementation.
- Actual scorer with refreshed E source/build provenance: **`8aff7fcb16c6b0767534f694f8ccf0f8d368dffddf935a754899e9b642ec9d74`**. Its source commit/tree/image/embedding receipt matched the manifest. Both executed binaries were unchanged before/after actual execution.
- Image `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`; offline2CPU/4GiB, single protected existing target, shared registry read-only, immutable source/binaries/tokenizer during actual run. No personal store/config/credentials/socket, network, new dependency or cache clone.
- Required scorer example build plus9 new/12 inherited checker tests: **14.853s**, exit0, peak4,294,967,296bytes, cgroup max2432/OOM0. Existing116 library warnings remain; no lint-clean claim. **No full Rust suite was run.**
- Actual consumer driver: **48.195972s**, exit0, peak526,741,504bytes; all memory pressure/OOM counters0. Driver/container retired; each attempted runtime child has a recorded exit, including expected termination after negative cases. Shared-host elapsed/memory observations are diagnostic, not a latency/performance comparison.

See [source map](evidence/source-files.json), [build receipt](evidence/build.json), [actual command](evidence/actual-command.json), [actual result](evidence/actual-result.json), [binary hashes](evidence/executables-after.sha256) and [semantic audit](evidence/semantic-audit.json).

## Acceptance and denominators

| Scope | Observed result |
|---|---|
| Complete default cohort |7 episodes /28 successful public queries /28 actual consumed answers. Raw and packed populations identical; retrieval labels9 scored /6 known-zero unscored /13 unknown. Five source/history groups retain the existing grouping contract. No answer-accuracy metric. |
| Declared middle answer-cache miss | Full cohort roster3 passed /1 failed /3 unattempted. The failed current query actually succeeded; its exact answer lookup missed. No generation/fallback, no empty-answer conversion, no cohort raw/packed aggregate. |
| Narrow refused output | Actual actor consumes the pinned refusal; typed unavailable result invalidates the member, no reward zero or successful answer. |
| Narrow malformed JSON and invalid UTF-8 | Actual actor consumes the exact pinned bytes; output-parse rejection, with raw bytes/hashes and actor results retained. These are distinct from corrupt artifact-hash preflight failures. |
| Narrow bad citation | Actor parses a valid record; independent parent check rejects its citation outside the actual included public payload. Parsed actor status and parent rejection are both retained. |
| Whole-cohort preflight | Invalid last member, old track with new fields and unsupported role version all reject before runtime output/provider/store. |
| Real-payload checker | A self-consistent forged context receipt rebuilt from altered payload is rejected against the actual public response before actor dispatch. Retained real role trace passes reconciliation;5 identity/correlation/outcome mutations reject. This is actual-artifact checker evidence, not5 new retrieval episodes. |
| Gold canary | Actual public requests/provider vectors, all current actor envelopes and answer outputs match the baseline member. Full environment and embedding-cache keys differ; role behavior is not inferred from hash equality. |
| Future-query canary | Earlier actor inputs/requests/vectors/outputs match. At the changed current fourth question the exact recorded key misses and stops; no normalization or automatic cache repair. |

Across the baseline, middle-failure cohort and six narrow single-member cases there were **17 attempted episodes,53 successful public queries/role attempts,51 actual answer-actor invocations,47 accepted role outcomes and6 rejected outcomes**. Two cache misses dispatch no answer actor. The17 attempted members have68 planned question slots, of which15 remained unattempted; this excludes the additional3 wholly unattempted middle-cohort members (12 question slots). Narrow cases intentionally execute one member after full manifest preflight; they are not reported as complete seven-member cohorts or independent quality groups. Unknown runtime outcomes observed0; implementation preserves interrupted outputs as unknown rather than rollback.

Public RPC/provider and private-control counters remain separate from role counts. Local answer elapsed intervals are not added to nested RPC durations. Synthetic recorded output bytes are measurable; live model usage, pricing and backend retries are unavailable. [Checks](evidence/actual-checks.json), [complete roster](evidence/default-roster.json), [middle failure roster](evidence/middle-miss-roster.json), [example current envelope](evidence/example-current-envelope.json), [role proof](evidence/example-role-proof.json).

## Failures retained and corrected

1. Initial host parser test accepted an escaped lone surrogate because JSON re-encoding escaped it instead of checking its decoded string. The9-test run had1 failure. Decoder now recursively requires strict UTF-8 for decoded keys/strings; subsequent host and actual isolated9 tests passed. This was a parser/checker defect, not a model or retrieval failure. [Original failure](evidence/failure-surrogate-parser.log).
2. Source57d5bb59 isolated002 check stopped before Cargo: four test-fixture cases referenced an old193 report `build.json` that was deliberately absent from the exact source export. Exit1, retired1.718s, OOMfalse; **no compile or actual consumer ran**. Test249ff40e constructs its test-only build fixture from exported Rust/Cargo source instead of adding an archived report dependency. [Failure](evidence/failure-exported-test-fixture.log), [receipt](evidence/failure-exported-test-result.json).
3. The expected runtime/cache/refusal/parse/citation/future failures above are named negative gates, not ignored failures. Raw outputs and typed outcomes precede acceptance assertions. No runtime, ranking, timeout, normalization, cap or cache-key policy was relaxed after outcomes.

## Remaining gaps and ownership

This is one actual E1 recorded-ANSWER consumer. Extraction/query-planning/judge consumers, transformed-memory spans/turn coverage, official LoCoMo/LongMemEval goldens, human proposition-support/judge calibration, native writer/keyed-retry/lifecycle supervisor, broader reproducibility, chronological budgets/reward and promotion remain separate A6 packages. Existing BEIR/tokenizer/controls/grouping implementations are preserved; no duplicate framework is needed. Post-query barrier is not a response ReadView; no reset/lifecycle/native/public2 guarantee is newly proved here.

The completed static SciFact300 result remains parity/no demonstrated gain with descriptive cost regressions and no promotion. This synthetic output consumer does not amend it or authorize another study, model, training run or paid call. Six integration closures do not mean whole A6 completion.

## Cleanup and retained inputs

No E raw roots are released before publication/results. Protect `/private/tmp/recalld-a6-recorded-answer-validation/001-source` (parser failure),002-57d5bb59-build (pre-build test-fixture failure),003-249ff40e-build (frozen build/source) and004-249ff40e-episodes (actual proof), plus the isolated code worktree/Git. Existing owned-native-controls cargo/target, shared unit-tests registry, tokenizer bundle, prerecorded fixture inputs and all empirical/failed inputs remain protected. No raw HOME archive or cache clone was created. After publication, only explicitly released completed disposable roots may be cleaned by the central owner; the compact source/results remain in Git and originalrepo/HOME copies.
