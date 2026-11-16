# Heterogeneous namespace vector owner

Status: validated, awaiting coordinating-agent publication review. No ordinary native
startup or public API2 activation. Proposed base is actual PR178
`3361ddb2309593493326ace621d4fdcc9ba89d8c`; the only additional parent is actual
PR175 socket-fixture correction `a54cdc7dc89221f66049147ee8543d84bbb49977`.

## Result and scope

One existing shared vector owner now contains canonical namespace partitions of
the unchanged Flat kernel. The actual five-namespace fixture has widths 4 and 5,
three explicitly remedied namespaces, one unknown namespace and one old Bound but
uncertified namespace. Independent publication now retains all five physical vectors
and exact f32 bits at G15, with all four required views at the actual canonical
GenerationRef. Real native purge removes the wide namespace and republishes four
remaining vectors; a subsequent rebuild preserves them. Corrupt referenced vector
files fail open without repairing or changing the canonical source.

The private constructor still rejects this heterogeneous fixture at `native_profile`.
Its profile loops are unchanged. The positive publisher result does not establish
ordinary native startup, per-namespace semantic admission or public API2 activation.
Existing canonical certification/profile consumers remain responsible for semantic
permission; partition membership grants none.

The public Rust shared-handle value changes from FlatVectorIndex to VectorIndexOwner
in Recalld, DerivedCompletion, shared/pinned vector registries, API/MCP adapters,
IndexHandles and graph helper signatures. Legacy direct callers construct
`VectorIndexOwner::uniform(dim)`. There is no heterogeneous dimensions sentinel or
new disk representation. The standalone Flat kernel, SIMD/filter/order/tie behavior,
CHVI serialization and physical vector store are unchanged. See the
[implementation note](../../implementation/heterogeneous-vector-owner.md) for exact
callers and migration limits, and [source-boundaries.json](source-boundaries.json)
for byte comparisons.

Legacy autolink and duplicate helpers require the uniform layout and return typed
errors for partitioned owners, including zero-cap requested autolink. The existing
certified native producer path remains its owner's canonical selection path. This
leaf does not compose Curie's later graph/producer leaves or redesign legacy
nonfatal advisory handling. Their actual joins must retain both the legacy-owner
guard and this uniform-layout fence. Future measured A2 coverage must count the
examined partition rather than the global owner total.

## Exact validation attribution

| Run | Exact source | Actual result |
| --- | --- | --- |
| before001 | `3f7bface8a9f5ef55d0000f3809086bc2769be92` | Compile-only E0107/E0053 in new fixture Result alias; no tests executed |
| before002 | `e7674702c7fac3e5018f5a3b6c2f83a65e4f40b5` | Two intended failures, 0 passed, 3.23 s: private profile rejection and independent publisher wrong width |
| candidate003 | `3f6f66efc6ada046f6f8691c252ea7b0a9b414ee` | Two owner tests and three heterogeneous tests passed; no full suite |
| candidate004 | `30e9d58f626c108f73cf962120405fb03c642b67` | Compile-only E0599/E0282 after uniform fence needed the removed VectorIndex trait import; no tests/full result |
| candidate005 | `3ec4ff51289e23bf217afbd1efa40a067d74e4fd` | Three owner tests and three heterogeneous tests passed; full default all-target run: 1,437 library tests, five CLI tests and ten example tests passed, two library tests ignored; fmt and all-feature/all-target Clippy exit 0 with warnings |
| candidate006 | `0f329f836033a23aa09cd9ea514d2652a3645983` | Approved one-line socket-fixture merge: two CLI tests passed in 1.59 s and fmt exit 0; no repeated full suite |

BEFORE002 canonical data stayed at G15 and all four view stamps remained unavailable.
The returned CommittedMutation contained an actual **prior G3 MemoryInserted** receipt
from pending derived work, not a newly committed G15 receipt. The before binary SHA256
is `184c2803ccfe343e466e00321399a8cd9b276deffd43044ed40164e6cc4465c7`.
The final startup test deliberately asserts the retained profile rejection; only the
independent publisher changed from RED to GREEN.

003 to 004 added the explicit typed legacy autolink fence and cleaned unused imports.
004 to 005 restored the needed VectorIndex trait import and added the zero-cap
autolink regression. Only 005 supplies the final runtime full-suite evidence.
005 to 006 changes exactly one test line after socket accept:
`connection.set_nonblocking(false).unwrap();`. Production, Cargo, examples and benches
are unchanged, as recorded in [the exact patch](candidate006/fixture-tail.patch) and
[comparison](candidate006/runtime-unchanged.json). The original Darwin OS35 failure
belongs to PR175's queue evidence; this Linux tail does not itself prove Darwin GREEN.
Final published-head CI remains required.

The 005 library run took 862.92 s. Its executable SHA256 is
`e1280c77d97be6ea7ca6fd90236ecab5427cfcd054186a8694915f506f66631a`.
The [actual executable inventory](candidate005/actual-test-executables.json) covers
all twelve `Running` test binaries plus the CLI child executable, captured before
cleaning this agent's package for 006. 006 has its own actual CLI/test executable
inventory. Source manifests were verified unchanged after each run.

Clippy is a successful compile/lint check, not all-feature runtime coverage and not
warning-free. In addition to existing warnings, the new fixture has large-error and
await-holding-lock diagnostics. The referenced fixture explicitly drops its storage
guard before the first following await; no source cleanup was mixed into the frozen
full result. No production locking change is claimed from this diagnostic.

## Reproduction and resources

Each run has its exact Docker command, source manifest, toolchain, exit code, logs,
memory counters and available executable hashes. [validation-manifest.json](validation-manifest.json)
hashes candidate003 through candidate006; the separate before manifest covers both
BEFORE attempts. These are compact repository evidence, not database/cache archives.

Runtime validation used the fixed recalld-ci image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
Rust 1.94.1, offline Cargo, Docker network none, two CPUs, 4 GiB, two build/test
threads, a read-only source snapshot and registry, and one reused private target.
No personal data, credentials, daemon or socket was mounted. The two named historical
BEFORE binaries were mounted read-only for the existing compatibility probes.
Each source switch first ran `cargo clean -p recalld` in this agent's target.

005 ran `cargo fmt --all -- --check`, the two named focused filters,
`cargo test --locked --offline --all-targets`, then
`cargo clippy --locked --offline --all-features --all-targets`.
006 ran formatting and `cargo test --locked --offline --test vector_certification_cli`.
005 memory peak was 4,294,967,296 bytes, memory.max events 1,333, and OOM/OOM-kill
counters zero. This indicates limit/reclaim pressure, not an OOM failure. No memory
increase or retry was used. 006 peak was 3,547,471,872 bytes, max events/OOM zero.

All owned run containers used --rm and completed. Raw validation roots remain
protected until a PR exists and results are recorded; central cleanup then receives
`/tmp/recalld-heterogeneous-vector-validation` for release under the user's retention
override. No raw archives are required. Keep the source worktree and the single
continuing target `/tmp/recalld-owned-native-catalog-ids-full-xk182ybj/target`, shared
registry and explicitly required historical inputs. No raw-root bytes have yet been
claimed recovered by this handoff. HOME notes and their byte-verified manifests are
recorded alongside this report.

## Review and next dependent work

Hume independently source-reviewed runtime 3ec4 and the exact test-only 0f329 tail:
unchanged Flat kernel, exact eligible IDs, namespace checks even at zero cap,
error-atomic maps/counts, canonical namespace layouts and retained publisher locks.
That is source review, not another runtime result. Huygens and Curie released the
mechanical shared-handle hunks before implementation; their WIP was not imported.

The next composed acceptance work uses actual validated query/producer consumers and
the approved A6 native sweep plan. Ordinary heterogeneous startup/profile admission
remains a separate gate. API1 native catalog/purge projection has an approved source
plan and sole runtime owner, but no implementation or wire proof is claimed here.
No additional broad test run is required for this documentation or the approved
one-line test tail; exact published-head CI must pass before merge.
