# A2 conformance inventory after PR140

Source baseline: published PR140 `5f0e4f02d68ff3b3ed56a899227e24cfdaf8f92f`, including PR133 and PR132. This is a source audit, not a new test run or serving-readiness certificate. The parent report records 13 focused checks, 1,041 library tests and two CLI tests passing; fmt and clippy passed with existing warnings. Specs remain read-only: CONTRACTS and INTEGRATION-REVIEW take precedence over A2-retrieval.

## Concrete remaining work

| Gap at this exact source | Evidence and next action | Ownership/dependencies |
| --- | --- | --- |
| Entity admission caps novel IDs, not total channel members | `pipeline.rs` asks for `2L + union.len()`, observes every returned entity match, then caps only novel candidates. Overlap can exceed the authoritative v2 total `2L`. Reproduce with actual entity/vector/FTS adapters, then cap v2 before union. Keep the explicitly named v1 novel-only allowance and ranking formulas. | A2, minimal child of PR140; no new storage or wire parent. |
| Vector/entity examination and exhaustion are unknown | `ChannelDiagnostics` deliberately retains `None`; `diagnostics_report_unknown_vector_exhaustion_and_measured_metadata_cap` asserts that boundary. Offered/admitted counts do not establish source exhaustion. Add actual measured source outcomes in a later bounded slice; never serialize unknown as zero/false. | A2 supplies measurements; Huygens maps public coverage. |
| Full section 9 explanation is absent | `ResultProvenance` reconstructs `max(own_score, inherited_floor)`, but does not decompose baseline factors, temporal factor, entity overlap/recall, minScore disposition, raw/effective strength or RIF multiplier. Next concrete action is collect factor evidence where scores are actually computed, with reconstruction/toggle-identity tests and an actual query consumer. | A2 internals, coordinated with Huygens's strict `explain` wire consumer. No fabricated explanation at activation. |
| Legacy FTS remains top-B before eligibility | `run_fts_search` selects v1 top-k/error fallback versus v2 eligible paging/error propagation. The refill/failure tests explicitly preserve that staging boundary. This is existing named compatibility debt, not proof that hard eligibility before caps is optional. Any change needs an explicit contract decision and actual baseline fixture; this entity slice does not change FTS. | A2 contract decision with A4; no silent semantic expansion. |
| Operator RIF is on a separate published sibling | PR76/89 implement off/shadow/experimental, pair/target/cumulative limits and degree-preflight examination bounds. PR140 does not include that sibling. Retain default Off; quality/performance promotion remains unmet. | Consume actual PR89 only when needed. Preserve the small synchronous ReadHandle/pinned `rif_mode` compatibility fix, not unrelated checkpoint ancestry. |
| Public selection/coverage and native admission joins remain separate | PR133/140 are internal selection consumers, not public v2 activation. Supplemental graph/supersession work occurs after initial pipeline diagnostics: overall coverage must use final diagnostics plus supplemental flags. Ranked `totalMatches` excludes the supplemental pool; historical omission is distinct from unresolved paths. | Huygens owns public selector/schema mapping; helper owns validated native provider admission. Forward admission through `pin_read_engine`; Pascal owns original API-origin recorder routing. |

The shared v1/v2 canonical eligibility and final-read corrections in PR140 are distinct from the versioned `retrieval-v2-default` channel budget. Hard filters remain before allowances in both versions; retaining v1's named candidate-budget policy does not permit stale cache eligibility, filtered rows consuming its allowance, or changed selected records reaching observers.

## Acceptance evidence and remaining limits

These are source/test mappings, not a claim that every transport composition has run:

| A2 section 11 item | Existing coverage / remaining gate |
| --- | --- |
| 1 inheritance | Pipeline and semantics fixtures cover injected/existing/stronger-own, minScore, multiple paths and temporal invariance. |
| 2 exact vector | Actual flat-index fixture rejects more than 1,600 ineligible higher-cosine rows and checks the eligible oracle. |
| 3 FTS | Actual SQLite refill, ordered ties, pinned cursor, exhaustion and 12,800 examination boundary; v1 fallback debt remains explicit above. |
| 4 entity | Namespace filtering, idempotent replacement/counts, canonical rebuild and autolink sibling evidence exist. Total v2 channel budget is the next correction. |
| 5 metadata | Full eligible scan and temporal/UUID ordering beyond the old early cap are covered. |
| 6 filters | Pipeline, structured-label and similarity fixtures cover canonical filters and projected strength. Full public mapping remains A4/A6 composition work. |
| 7 versions | 0/1/64/65, branch/cycle/missing/foreign/filtered successor, current/history and inherited-floor fixtures exist. The 65th examined successor proves the 64-traversed-hop limit and consumes shared work. |
| 8 graph | Depth, diamond/cycle, reciprocal directed edges, ineligible relays and whole-neighborhood budgets are covered internally. Supplemental relations have separate diagnostics and no invented terminal edges. |
| 9 RIF | Published sibling correctness evidence exists; join and matched-strata quality gates remain separate. |
| 10 ties/explain | V2 stable tie fixtures exist; full explanation reconstruction/toggle proof remains open. V1 graph representation sensitivity is a known compatibility limit, not fixed by PR133/140. |
| 11 failures/readiness | Source/FTS/nonfinite/final canonical read failures and coherent selection tests exist. Actual public native/readiness composition is owned by the admission and transport joins. |
| 12 no-write | Real canonical state/observer tests exist, including final hydrated exposure IDs and per-namespace groups. Complete HTTP/MCP/daemon v2 conformance is still an integration gate. |

## Minimal order

1. Child PR140: actual entity-overlap RED, v2 total `2L` correction and honest admission diagnostics. No public activation or formula change.
2. A2 score explanation collection/reconstruction with Huygens's real public consumer; freeze the necessary parent when his selector implementation is ready. Keep unknown coverage nullable until real scan measurements exist.
3. Measured channel coverage and explicit legacy FTS contract resolution, followed by actual transport/oracle conformance with A4/A6. RIF sibling joins only when consumed; optional quality gate stays unmet.

Canonical namespace scans remain a material cost. Prior failed A5 instrumentation/sampling and incremental-publication gates do not establish performance promotion for this source. No new benchmark or default-enablement claim follows from this inventory.

## Resource handoff

PR140 completed raw runs and its old target were released to the central cleanup owner, who confirmed deletion of all 13 named paths (batch 11). The observed concurrent free-space change was 6.99 GiB, not exclusively attributable to this release. Tracked reports/source and the sole private `.validation/cargo` cache remain. The next executable slice uses one fresh bounded target; no per-run cache clones or new HOME raw archives.
