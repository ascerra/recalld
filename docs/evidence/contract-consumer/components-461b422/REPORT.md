# Interim consumer component checks, not activation proof

Runtime461b422 on frozen API/doc9101522 and prepccdddee. This is an intermediate
local component commit for the intended joined MCP/REST consumer, not a helper
PR. Backend capture/current-validation and native serving are not yet joined.

Docker recalld-ci:1.94, Rust1.94.1, networknone,2CPU/4GiB, CARGO_BUILD_JOBS=2,
only assigned source and this agent's private cargo/target/output mounts.
After preserving prep binary hashes, cargo clean --offline -p recalld ran in
this agent's OWN target before the source-root switch. Then cargo fmt --all,
cargo test --offline --all-features exact_, and the same command filtered by
discover_. **13 exact-filter tests passed in4.45s and7 discovery tests passed**.
The former includes5 new component tests plus existing exact-vector/profile
checks. This is not a full-suite run for the consumer branch; the parent prep
fullsuite833+2 remains separately recorded. Executed binary hashes are preserved.

Pure fixtures include205 UUID-ordered one-hot vectors in four disconnected,
dense>10 tied groups; exhaustive independent pair enumeration matches every
page budget1,7,101,100000, including the pair across index0/204 and all20910
comparisons. Other checks cover empty/singleton/no-match progress, overflow,
invalid positions/vectors, strictscan fields/nulls/ranges, cursor codec/bindings
and unchanged live version1-only advertisement. No real snapshot generation,
restart/stale guard, Ghost/invalid storage proof, native publication or
transport2 acceptance is demonstrated here. Cursor generations are authored
codec fixtures only. No200/2000/20000 performance gate is claimed.

Ordinary contract2 remains failclosed. Public tool guidance describes current
v1 limits and no v2 activation. The backend API/document and concreteconsumer
plan name the remaining tests and actualnative/writer prerequisites.
