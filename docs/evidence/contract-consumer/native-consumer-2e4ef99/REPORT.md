# Gated exact-scan consumer and v1 context: frozen validation

Runtime: `2e4ef993fca493d3fbc5e9513213221f64a70646` on `impl/contract-consumer`.
The seal commit changes only documentation/evidence. Public application contract2
remains rejected before backend work. This is a bounded A4 slice, not A4 completion.

## Actual dependencies and behavior

PR82 native readers, including frozen phase-clock correction `a69865b`, is the
review base. Additional real dependencies are PR13 structured MCP results,
PR19 existing session resource/legacy coverage, and PR70 writer/publication
guards plus the concrete canonical-vector-snapshot backend `ccf8636` (backend
parent `af20d73`). PR69 canonical labels arrives through the native reader base.
No new batch, catalog-policy, coherent-query, or unrelated CLI WIP is consumed.

The comparator visits canonical UUID pairs in order, uses f64 cosine on original
f32 bits, and binds continuation to actual snapshot generation, namespace,
profile, threshold and eligibility policy. Every page captures again and checks
publication/generation afterward. Valid Ghost vectors remain eligible;
tombstones do not. Missing vectors are reported exclusions. No fake ReadView or
latest-counter token is used. Overflow produces a whole typed error; no matches
are silently dropped. The 8MiB wire bound includes actual MCP text plus structured
copies, JSON-RPC and daemon framing, as well as REST serialization.

The existing v1 session resource and new GET /capabilities report actual store
identity and validated provider configuration identity when available. Unknown
provider provenance stays unknown. Remote context uses one dedicated strict RPC
with no weaker fallback. These fields do not attest namespace provenance,
canonical schema version, remote weights, generation, or readiness. Both
surfaces advertise only contract1. Existing v1 field shapes/defaults/coverage
remain intact.

## Final execution and provenance

Commands: `cargo test --offline --locked --all-features`,
`cargo fmt --all -- --check`, and
`cargo clippy --offline --locked --all-features --all-targets`.

All exited0: **972 library tests and2 CLI tests passed**,4 doctests ignored.
Clippy emitted warnings. The library run took410.13s; CLI tests took2.68s and1.43s.
Logs and status are in `final-phase-clock/`; `final-manifest.json` hashes the
stored artifacts and their uncompressed contents.

Environment: image `recalld-ci:1.94`, recorded imageID
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
`RUSTUP_TOOLCHAIN=1.94.1`, `CARGO_BUILD_JOBS=2`, Docker network none,
2 CPUs/4GiB, source read-only at /work, private cargo and target directories.
No home, credentials, or Docker socket mounts. `container.txt` records mounts
and resource restrictions. All222 pre-run source hashes matched after checks;
all6 executed test executable hashes are retained. No source-root switching
occurred during consumer validation. The earlier prep-to-consumer root switch
used package clean; its separate evidence was already retained.

## What the fixtures establish

Pure exhaustive205-record oracles cover20910 comparisons, cross-page pairs,
dense groups beyond10 neighbors, ties/disconnected groups, small budgets and
no-match progress. Analytical near-threshold tests distinguish cosine from raw
dot even within the actual Bound unit-norm tolerance. Scaled-vector unit tests
do not claim arbitrary nonunit vectors are valid Bound storage.

Actual native tests use real legacy phase-loss receipts to produce a Ghost,
then actual migration/attestation, native insertion/tombstoning, derived
completion and reopen. They cover205 eligible rows plus a tombstone, all20910
known dense edges across10000/10000/910 comparison pages, byte-immutable reads,
MCP text/structured and REST inner-handler parity, oversized-page rejection and
smaller retry, restart continuation, between-page invalidation, and a real
feedback commit after capture that discards the page. No schema/provenance rows
are fabricated. The fixture panics if the reader accesses the provider.
Public MCP/REST contract2 still rejects. Inner-handler native proof is not public
activation proof. Serializer stress fixtures separately exercise actual
transport encoders with authored worst-case fields.

Five discovery tests cover actual known/unknown provider identity, HTTP/MCP
context, source-byte immutability, and framed RPC with old/malformed peers.
Hume independently reviewed comparator math; this is not an independent test run.

## Preserved failures and corrections

`interim-manifest.json` covers46 earlier artifacts. Compile failures are retained,
including a mistaken Vec unwrap, missing fixture arguments/imports, a missing
legacy mutation guard after the native join, and private handler visibility.
The guard correction preserves one admission/publication guard across commit
and effect application. Known pending completion retains its full receipt.

The interim full run passed905 tests and failed1 obsolete orphan-posting fixture.
The corrected fixture supplies a real foreign canonical record under the stale
target posting's UUID and retains exact owner/posting/count assertions. Its
focused run and final full run pass; production deletion was not weakened.

The first discovery harness assumed connect automatically sends ping. Four tests
passed and the fifth blocked; only the owned container was stopped (exit137).
Explicit ping and a bounded server join fixed the harness. Its actual failed
binary hash was captured before relink. The associated source manifest is
explicitly a reconstruction, not a contemporaneous capture. The corrected5
tests and21 exact-related tests passed before the final suite.

The original native scan baseline `6a71248` passed but had Pascal's subsequently
discovered regressed-clock forget bug. The final runtime consumes `a69865b`,
preserving absent immutable historical proof semantics while new forget writes
use the explicit monotonic marker rule. Earlier passing evidence is not used
to deny that bug.

## Limits and remaining scope

The small D4 native fixture captured206 rows/143718 encoded-record bytes and
3280 examined/copied vector bytes per page. Baseline capture times were
50.862808/50.532140/51.813061ms; comparison times1.150295/1.133754/0.185709ms.
These debug correctness observations are not the200/2000/20000-row A5
performance gate, particularly not20k/d1536. Repeated snapshot copying can
dominate paging. Current bounds are100000 rows/256MiB with whole errors.

Remaining A4 includes actual public native write/profile selectors, CAS/keyed
batch adapters, full operation schemas/envelopes, coherent filtered list/catalog
pagination, compatibility diagnostics, remote exact-scan RPC, and activation
only after whole writer/serving gates pass. This slice introduces no implicit
unbounded cursor cache or offline CLI to publish helpers.
