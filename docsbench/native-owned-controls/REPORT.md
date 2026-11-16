# Native prefix checkpoint and serialized false-read consumer

Frozen `392034d5c125d8d4759ffc17b43dfad439e7e9da` passed full1222 library+2 CLI+3 scorer tests,20 Python checks, format, all-feature/all-target Clippy (warnings retained), and the final actual native episode. Initial actual consumer `ac16b6d4db0f3ea0772ced3b0b4287caeca5ddb4` passed; the final tail adds one Python manifest test and invocation documentation. Runtime/Cargo/example bytes are unchanged from `000f7c12fc6259bd745037aa2fca99ffc491f6b7`. Publication remains a separate reviewed handoff.

## Behavior and actual dependencies

The private `serialized-native-prefix-synthetic-diagnostic-v1` consumer admits an existing native schema2 prefix through the same owned `StorageEngine`, initializer, derived completion registry and production MCP bridge. It validates canonical owner/schema/scope and Bound profile compatibility before text-query provider use. Ordinary startup retains its gate. Actor discovery exposes only get/search; explicit `recordAccess:false` is required. Missing/default-true or explicittrue access intent and store/feedback/forget requests reject before dispatch. Sweep and reset RPCs remain unsupported.

Actual parents are PR138 `05e0d55aa3a92483e294b2a5e79cf490ce270609` plus PR130 `e571104a2fc551a29314f6361b9b67503acd1fdb`. PR138 supplies native allocator/catalog/context ownership; PR130 supplies the existing115 checkpoint/restore and117/125/128 runtime/control ancestry. PR128 alone was insufficient. Join `5d3f14f` preserves both native batch and exact-scan fixture modules; duplicate `AccessRecorded` validation from the merge was removed without changing behavior. No broad checkpoint, insertion-wire WIP, selection candidate or ordinary startup selector was imported.

Feynman assigned this shared initializer extraction to this worker. `Recalld::with_owned_storage` consumes the already-owned engine and uses the existing registry/cache/provider/query construction. The private native constructor requires explicit disabled automatic sweep, autolink and RIF. The native query flag is forwarded into the existing pinned query engine; future PR140's extracted pin constructor must also forward it when joined. Owner/schema/profile checks remain in the actual canonical read view, not an API-number bypass.

Checkpoint capture and restore extend the existing typed native logical walker and existing115 owner/ancestor/source/destination/cache/allocator protections. No random export envelope or normalized hash format is introduced. Native manifests name schema2 and `native-logical-export-v1`; legacy checkpoint manifests retain their original absence representation.

## Source review map

Frozen `392034d` locations: `src/system.rs:290` shared initializer; `src/bench/native_prefix.rs:42` strict owner/profile admission and `:145` actor dispatch; `src/storage/metadata/publication.rs:200` canonical owner/scope/profile check; `src/search/pipeline.rs:701` provider boundary; `src/search/read_view.rs:230` pinned flag forwarding; `src/storage/logical_export.rs:423` existing typed native walker; `src/bench/checkpoint.rs:221` strict native/legacy contract dispatch and `:362` same-core restore constructor. These source anchors complement actual execution; they are not standalone runtime proof.

## Actual consumer results

The four-source fixture is constructed by real insertion and schema migration, with source mappings and actual receipts retained. It is acknowledged checkpoint provenance, not live actor ingestion or independently deterministic native genesis. Baseline plus three fresh restored processes execute15 actions each, comparing exact public outputs/order/exposed scores, provider request/vectors, generation/hash and owned clock/allocation state. Each real post-step barrier verifies all four derived generations and pending0. False reads and rejected actions leave canonical/runtime state unchanged. Source and checkpoint byte canaries remain unchanged.

Two additional actual suffixes perturb evaluator labels and future questions. Changing labels changes manifest/cache identity while actor inputs, outputs and provider vectors remain identical. Future questions do not appear in actor inputs/code, wire or provider requests before their current step; changing the future schedule preserves earlier actions, outputs and vectors. The separate actor process inherits only public stdio, never private control pipes. Six suffixes total90 actual tool actions, plus one separately blocked text query. Offline replay misses error; no fallback or generation occurs.

Ten actual negative cases cover a live original owner, a blocked query worker, unsupported checkpoint clock, existing destination, symlink destination, wrong manifest digest, wrong source, unbound provenance, wrong model and wrong revision. Constructor failures make zero provider calls. Existing destination/source/checkpoint sentinels remain unchanged; all successful owners and actor workers are joined before subsequent successful restores.

The real synthetic scorer consumes four query events: two scored, one zero-label unscored, one unknown annotation. The reconstructed four-source ledger has12 events and no pending sources. These are synthetic contract results, not upstream dataset/scorer goldens or semantic quality. Public exposed scores/order are compared exactly; no public BM25 oracle is invented.

## Validation attribution and retained failures

| Frozen source | Actual outcome |
| --- | --- |
| `65450ae504ee3347fb1e266c3690590d4e48ca19` | Compile failure: new test assumed unsupported `SearchQuery: Default`; no runtime claim. Duplicate merge match-arm warning also retained. |
| `000f7c12fc6259bd745037aa2fca99ffc491f6b7` | Two focused admission tests passed;1220 filtered, not full coverage. Bench feature compiled the real constructor/hash/control changes. |
| `ac16b6d4db0f3ea0772ced3b0b4287caeca5ddb4` |19 Python checks, actual example build and native consumer passed, including three restores/two canary suffixes/ten negatives. |
| `392034d5c125d8d4759ffc17b43dfad439e7e9da` | Full1222 library+2 CLI+3 scorer tests,20 Python checks, format/Clippy and actual native restore/canary tail passed. Same runtime, additional Python manifest rejection test/documentation. |

The same final binaries also passed one existing legacy sweep/reset lineage with three restores and seven negative cases. Ten actual CLI manifest mutations (version/schema/public2/RNG/fullA6/writer/sweep/startup/executable claims) all rejected before the episode output directory existed. This regression adds no Rust rerun, and its source/binary hashes remain identical to the final native run.

The full library suite has0 failed/ignored/filtered tests; no doctest coverage is claimed. `evidence/final/verification.json` identifies actual test executable paths from Cargo logs and their immediately recorded post-run hashes. Actual consumer executables are checked before and after execution. The library test binary SHA256 is `58f30ae2a32759a32a5d0256ac0bb796683050f318e91ad95f9e05fb7d25d884`, identical to the earlier focused binary because runtime inputs are unchanged.

All validation uses immutable source mounted read-only, image `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`, offline Docker,2CPU/4GiB and one build job. A single private Cargo/target is reused, with the existing registry read-only; no full cache clones, personal data, ambient credentials or daemon sockets. Exact source/executable/command/log hashes accompany the final result. Compiled provenance changes require rebuilding actual examples; prior passed library checks are not attributed to another source.

## Limits and continuation

This is the first bounded private native checkpoint consumer, not ordinary serving, native API1 writer activation, public API2, independent startup determinism, deterministic all-RNG control, response ReadView guarantees, full A6 or quality/performance promotion. The read-only wrapper is a declared diagnostic capability boundary; it does not replace missing production writer adapters. Actual native writer and sweep suffixes remain the next dependent work after validated adapter/control joins. A full A6 acceptance-table audit follows sealing/publication and must distinguish implementation gaps from rights/cost-approved experiments. No paid model calls, training, dataset downloads, invented revisions or licenses are used.

Raw scratch remains protected at `/tmp/recalld-owned-native-controls-validation` until publication and recorded results, then central cleanup may remove completed runs under the user's retention override. Worktree/source/Git/reports and any explicitly retained single future cache remain protected. No raw HOME archive is required. Invocation details are in `../native-prefix-consumer.md`.
