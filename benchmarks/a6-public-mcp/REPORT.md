# Actual serialized public MCP diagnostic

Validated source: `6b29a4df677e0dd20e3a938ba950f676d4caf00d`. Actual dependency join `52c5309` combines PR85 scorer `70012c8` and PR88 validated v1 session-context runtime `2e4ef99`. No moving Halley core-control branch is imported. PR85 is published at https://github.com/calebevans/recalld/pull/85 with all six CI checks green.

This is the explicitly named `serialized-mcp-synthetic-diagnostic-v1` consumer: manifest/sourceMapping/trace1, public contract1, system clock, no reset/barrier/coherent-readiness guarantee. Canonical schema is null/`unattested-via-public-v1`; backendContext proves observed store identity and configured embedding identity only. Native schema/public-v2/full-A6 requirements reject before output/store creation. These are documented gates, not implemented capabilities.

| Actual check | Result |
| --- | --- |
| Python boundary checks | 7 passed |
| Rust examples/shared scorer | 10 passed (6 internal, 1 probe, 3 public scorer) |
| Existing library source unchanged from6377 | Reused exact passing 25 session, 3 structured and 3 performance tests; source/Cargo hashes checked before reuse |
| Format / repository Clippy | Passed; warnings retained, no broad library suite repeated |
| Baseline serialized MCP episode | Passed: 15 actor actions, 6 sequential deliveries, 4 public stored-UUID mappings, 12 source events |
| Actual invalid-manifest cases | 17 rejected before episode output/store creation |
| Same-binary gold-label perturbation | Passed: all 15 MCP requests, 6 input projections, executed actor code, 9 provider request bodies/statuses and cache artifact bytes unchanged |
| Future-query canary | Absent from actor code and earlier input/wire/provider channels; appears only at current-question delivery ordinal4 after prior ACK |
| Replay miss / private channel | One expected HTTP400 provider miss per episode with public tool error; private evaluator method rejects on actor MCP, guard descriptor absent from every actor process |

The supervisor keeps the future schedule and evaluator labels in private artifacts. Each actor invocation runs the separately copied, source-hash-verified `public-mcp-actor.py`, which imports only standard-library modules and has no fixture/evaluator/schedule factory. Ingestion gets fixed-state sources; subsequent processes get only the current question after the prior step acknowledges completion, all against the same owned production MCP session. Only the current recorded query-vector entry is loaded into the replay endpoint at its delivery boundary. There is no invented clock, second engine, alternate ingestion path or control implementation.

The actual public `recalld://session` resource supplies store/profile identity; the client does not reimplement discovery. Public tool calls use production JSON-RPC parsing/dispatch/adapters. Source mappings use UUIDs returned by successful public stores. The shared Rust scorer consumes these actual mappings and evaluator-only labels; accepted ledger events must match mappings. It reports 4 query events: 2 scored, 1 zero-label/unscored, 1 unknown. Synthetic scored-event means are Recall@5/10/20=.75, binary-source nDCG@10=.8065735963827292, extraction retention=1, all-required=.5. Changing all gold labels to the foreign source makes all4 queries scored with recall0, while returned-source ranks remain `turn-0` for every question.

Raw generated UUIDs, timestamps, responses and errors are retained. Output comparison uses actual per-run UUID-to-source mappings only for source ranks; it does not claim byte-identical public responses or deterministic reset. Actor process projection is tested, not an OS sandbox against malicious code in the same container. Fixed-state ingestion is explicit; there is no chronological-dialogue claim. No upstream scorer/dataset/license/revision, semantic quality, paid model call, full-A6 or public-v2 claim is made.

Runtime binary SHA-256: `d77dab88431bf14e9e8f23d42d5966dbd8863c7cc9885e41f75d44f6ed01343a`.
Scorer binary SHA-256: `52028b1daaa6e20b3de758df4efac6db4e66a564e0bf33d79ca1fbeb8dce0cb8`.
Manifest SHA-256: `e6abef5e92e3b6b8b0f2096e0a9906bd1cfc92bfef84b86e42503002cfe664f7`.
Executed actor-code SHA-256: `5baa8b7a31c7184170fedacb1b8162e3668e66eda96dd7cefb649b181f24fb2a`.
Pinned image: `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.

The target command was `python3 scripts/public-mcp-evaluation.py host --cache /tmp/recalld-canonical-harness-test-a1hpl2g2/cargo --build-cache /var/folders/lg/pd7snpz15137ng972h102kqc0000gp/T/recalld-public-mcp-zwoern3z/target --library-evidence /var/folders/lg/pd7snpz15137ng972h102kqc0000gp/T/recalld-public-mcp-zwoern3z/output`. It copied private caches, used network-disabled Docker with2 CPUs/4GiB, isolated home/config/data, explicit loopback replay service, and no personal credentials/socket mounts. The supplement invoked `validate_episode.py --root /work --out /validation/output` with frozen source/binaries read-only. Clippy reused the private target; binary hashes were checked unchanged afterward.

Preserved first source6377a88 contains both boundary defects identified by orchestrator source review: the actor received the complete future-query array, and its fixture validation called an evaluator-bearing factory. Its source is retained verbatim in source-boundary-baseline-6377a88.py. That run stopped on an independent example compile error (`profile_id()` returns String errors requiring explicit anyhow conversion), before any episode. It is source-audit evidence of the boundary defect, not runtime leakage proof. The corrected6b29a4d structure was accepted before runtime validation; the separate actual checks above now establish the bounded canary claims. Earlier checker/validation-scope assignments were also orchestrator coordination, not explicit end-user findings; frozen historical records are preserved rather than rewritten.

A mistyped prior-cache path was rejected before scratch/build/store creation and corrected without code changes. Inherited evidence whitespace is preserved. All failure logs and command context are retained in failed-6377a88.tgz/setup-failures.txt; passed-6b29a4d.tgz and actual-canaries-6b29a4d.tgz retain actual inputs, outputs, wire attempts, source mappings, actor code/projections, scorer results and negative cases. artifacts.json records every archive/member hash and original root; every member was re-read and verified. SHA256SUMS covers this folder without self-reference.

Next actual control integration remains with Halley's validated same-core private endpoint and Huygens' public activation. A current canonical GenerationRef from private inspect must not be relabeled readGeneration/readiness; logical hash/reset/clock/RNG requirements remain unavailable until their actual implementation and validation.
