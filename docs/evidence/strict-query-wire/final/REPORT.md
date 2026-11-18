# Prepared strict query consumer — sealed validation

Runtime: `e8754931272dfd77d8762092924bd7b60351a958`. Actual parents: PR146 `2d09509905fc2893bc54a3fad1ea2df0b6d5a3de` and PR88 `3961f6653d6c3076bdf9b5b8c27fa6b01a8f0b2e`. Base146 includes142/140/133/132; PR88 supplies existing structured wire/session/snapshot interfaces. No141/144/catalog/list/startup convenience parent.

## Exact validation attribution

| Source | Checks | Result |
| --- | --- | --- |
| Full007 `d2d2a83da6bf81f3d047337b374afbd3bcdb9f4b` | offline all-features normal-parallel suite | 1,096 library +2 CLI passed; 4 doctests ignored; exit0 |
| Same full007 | focused query, formal JSON Schema2020-12, fmt, clippy all-targets | 14 focused passed; 42 schema cases + mutation negatives passed; all exit0; clippy warnings retained |
| Schema008 `e8754931272dfd77d8762092924bd7b60351a958` | focused library tests + formal schemas + fmt | 14 focused passed; 58 schema cases + mutation negatives passed; all exit0 |

The only post-full production-source delta is two schema end-anchor patterns: generation and ep1 profile ID. Runtime parsers, selection, observation, receipt serialization and publication are unchanged. Tests add exact LF/CR/CRLF/U+2028/U+2029 negatives and typed Generation rejection. No final-head whole-suite or clippy rerun is claimed; exact published-head CI remains required.

Both snapshots contain470 source files and matched their pre-run hashes afterward. Full007 records nine executed Cargo binaries; schema008 records its executed library binary. Source-specific package cleaning occurred only after preceding executable hashes were captured, in this agent's own target. No before/after source-root switch reused an uncleaned package executable.

Image `recalld-ci:1.94`, installed Rust1.94.1, actual image identity: `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`. Containers used network none, 2CPUs, 4GiB, CARGO_BUILD_JOBS2, immutable source mounted read-only, one private reusable Cargo/target pair, and no home/credentials/socket mounts. Pinned Python packages were fetched separately and only executed offline; hashes are in tooling/wheel-sha256.json.

## Actual boundary evidence

Prepared MCP and REST inner handlers execute the owned coherent query/similarity path. They share strict input validation, explicit profile binding for supplied vectors, current/history selection, exact hydrated labels, original2 exposure receipts and current publication barriers. Metadata-only and supplied-vector fixtures avoid provider calls; false reads preserve canonical bytes. These are prepared handler/actual-owner tests, not live contract2 routes or general native startup proof.

Recall explanations project only PR146's actual ranked-query baseline-v1 evidence, currently RIF Off. On/off leaves selection, score and exposure classification unchanged. Similarity retains cosine/current-max semantics, including own-vector versus inherited floors; explain:true returns matching unsupported errors before exposure for false/true access flags. No fabricated similarity decomposition, score formula or profile label is added.

The actual24-neighbor fixture filters12 ineligible rows before retaining10 of12 eligible neighbors; the response reports final omission/cap counts and keeps explanations confined to ranked hits. Final post-supplemental graph/chain diagnostics and unknown channel measurements remain distinct; unknown is null, not an invented count or exhaustion claim.

Actual4MiB stored payload selection is rejected before false/true observation;3MiB selection succeeds with its full receipt inside the doubled MCP/frame budget. The width proof uses typed VALID LifecycleState/NativeLifecycleWrite transitions, full per-namespace receipt overhead, complete100-receipt failure projection and single-receipt duplication. It is not a claim that every constructed maximal-width history is reachable. Known final-barrier failure retains the full receipt, scope and readGeneration; repair does not advance canonicalG. Similarity across namespaces retains every separate receipt rather than inventing an atomic query receipt.

Native bound fixtures are actual schema/lifecycle migration and attestation with explicit owned test admission; canonical labels retain legacy_retained_only provenance. This does not reclassify retained labels as native provenance or prove ordinary serving activation.

## Failures retained and corrected

Full007 schemas genuinely accepted generation `1\n`; the same end-anchor defect admitted ep1+64hex+newline. The BEFORE formal regression fails on recall-explained.json/1\n, independently of the unchanged full suite. Schema008 uses strict end-of-input lookahead and rejects LF, CR, CRLF and Unicode separators without changing typed parsing.

Draft006's first formal checker failed during import because typing_extensions was missing from the test-only Python3.11 environment; no schema checks ran in that attempt. Pinning/extracting typing-extensions4.15.0 separately fixed tooling; the same immutable006 source then passed41 formal cases. Earlier draft001/003 compile-only visibility/fixture-field failures and prior focused passes are described in draft005/REPORT.md; no behavioral proof is inferred from those failed compiles.

## Scope and remaining gates

Live APIversion defaults and payload shapes remain v1; explicit public2 still fails closed. Prepared schemas are not advertised as live tool outputSchema/capabilities. This child does not complete A4: remote query/exact-scan RPC conformance, native generated-text admission, keyed query exposure, experimental RIF evidence, cosine explanations, ordinary startup/writer closure, and A5 performance gates remain separately owned. Current query/similarity exposure uses the actual132 recorder already included by142, so144 is not a dependency merely for its lifecycle tests.

Next owned consumer: actual API1 store projection from frozen148, preserving original1 context, native supplied profile requirements, generated document recipe and legacy shapes. Pascal owns lifecycle/client capability hunks; Curie owns internal legacy compound preparation. No merge or deployment is authorized here; publication handoff freezes this branch for Halley.

## Cleanup

No HOME raw archive. After publication and this recorded result, completed run/query-format roots may be released to the central cleanup owner. Preserve the worktree/.git/code/reports and the one active private Cargo/target for the next scope; do not recreate released paths. Exact release mapping and byte totals are sent after publication.
