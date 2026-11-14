# Private exact-scan snapshot backend validation

Runtime `b0c23113f6f61cdfa14701adae66a7b6eea4bdfe`, parent PR70 `af20d73`.
This private backend folds into Huygens's actual MCP/REST exact-scan consumer.
The committed API handshake is in that consumer's
`docs/implementation/canonical-vector-snapshot.md` (initial doc `9101522`,
subsequent private accounting amendment). There is no standalone helper PR,
public fixture escape, native serving activation or whole-query ReadView claim.

Seven focused tests pass: actual insertion staleness and owned f32 bits/order;
actual policy and Observed writes with unchanged effects_epoch; terminal rows
counted against limits; overflow; corrupt referenced slots; pending canonical
effects; explicit namespace policy and lineage revalidation; retained Ghost and
invalid-vector accounting; bad reference digest; actual metadata publication
lock exclusion. The Ghost/invalid fixture is explicitly cfg(test) legacy data;
it does not activate a native engine. Native ownership/binding integration,
restart cursor oracles and actual 200/2,000/20,000-record performance checks
remain consumer prerequisites.

Formatting and all-target clippy exit zero. Clippy reports 481 library and
526 test warnings (459 duplicates), plus one binary warning. No new full-suite
result is claimed: the backend proof is seven focused tests plus all-target
compilation. Initial five-test run passed; the expanded test initially failed
compilation because an assertion's sum lacked `usize`, then the corrected
seven-test run passed. All original logs are retained.

The snapshot borrows mmap bytes, checks budget and ownership before vector
copy, and counts every selected row including exclusions/tombstones. Private
accounting distinguishes encoded row bytes, examined retained vector bytes
and Available-vector copies. It is not physical I/O, allocator-exact memory
or a performance claim. A post-comparison generation/lineage/namespace-byte
check remains mandatory in the actual consumer.

Exact final inner command:

```sh
cargo fmt --all && cargo test --offline --all-features canonical_vector_snapshot -- --nocapture && cargo clippy --offline --all-features --all-targets && cargo fmt --all -- --check
```

Container: `recalld-ci:1.94`, Rust toolchain `1.94.1`, network none, 2 CPUs,
4 GiB, CARGO_BUILD_JOBS=2, CARGO_HOME=/cargo, CARGO_TARGET_DIR=/target,
CARGO_PROFILE_DEV_DEBUG=0, CARGO_PROFILE_TEST_DEBUG=0. Source mount
`/tmp/recalld-implementation/canonical-vector-snapshot:/work`, dedicated fresh
target `/tmp/recalld-canonical-vector-snapshot-validation/target:/target`,
readonly registry `/tmp/recalld-unit-tests-l__ja1yo/cargo/registry:/cargo/registry:ro`.
No home credentials, provider network, daemon or socket mounts. Automatic
approval review rejected cleaning a prior validation target; it was untouched,
and this proof used the new dedicated target throughout. No source-root switch.

`manifest.json` contains all source-file hashes and six actual executable hashes
selected by the final test log. Manifest SHA-256:
`1c08ec5083b2560401fa3df094c1af870bdb8bcc2498a6fe9ac486a027fd2c39`.
Final log SHA-256:
`754c535509e002d56d291b25b15024c98c1c59a8a131b2591dde2804e8e26877`.

## Reviewed namespace alias correction

Runtime `7e276050b532d610d1d7db88b27fff47fba23392` preserves existing exact-first
ASCII-case-insensitive namespace lookup inside the same canonical transaction.
Ambiguous fallback fails closed. Added actual catalog creation, restart binding
continuity and exact-match/ambiguous legacy-shape regression. Final **8 focused
tests pass**, formatting and all-target clippy exit zero, same warning counts
as the initial backend. No API type or activation change.

Alias manifest SHA-256: `95aebbfc1b6a8554d3960a9364e430dd70f3a6a6813af766cd4cab99480a8028`.
Alias log SHA-256: `0f5f7ffd7e09a1aac515f154435556a690472aca4b0d011747b28bfe486b015e`.
Initial source/binary/log evidence remains unmodified.
