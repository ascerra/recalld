# Native request context: validated consumer handoff

Validated source `a9053ef1287b0b2a1a266c7c72268ad61f84291b`, tree
`fa4fed7d18d206304edcc15984cad0713289a09a`, actual parent PR121
`3fa91d4e542ec8ed5519fe4c3b87e9e665043a40`. Later evidence and planning commits
change no Rust, Cargo or test sources. Publication requires coordinating-agent
review of the final head/report/HOME manifest, followed by an exact READY handoff
and branch push freeze for Halley, the dedicated integrator.

## Implemented behavior

Native catalog and numeric policy accept the existing closed API1|2 context
without changing their native owner/scope guards or hash domains. Purge selects
its canonical codec from owned store mode independently of the originating API.
The existing MutationContext validator and standalone native_records,
native_lifecycle and native_maintenance modules are unchanged.

The new shared retained context is consumed by actual native purge. API1 start
proof retains original force/CAS; known historical native API2 omits the field,
preserving old bytes. Missing proof must match exactly one of four original
force/CAS hashes using original previousG, never fork currentG. One canonical
helper validates start identity for strict offline/startup reads, current reads,
resume and replay; child hashes derive from that validated origin. Null, unknown
or altered proofs fail closed. This is not a general API inference mechanism.

Fresh CAS equals the transaction's current generation and the start receipt
advances it once. The start proof checks previousG+1 equals committedG. Genuine
BEFORE artifacts prove both force values with absent/present CAS, including
pre-child recovery and completed/forked histories without receipt rewriting.

## Combined owner, transaction and receipt review

The private native-owner mode is constructor state established after schema2,
lineage and selected-text preflight. The accepted worker holds its publication
lease, graph guard and storage write guard while checking this mode. It then
enters the existing purge mutation guard, checks handle/accepted-work availability,
and compares decoded current control schema with owner mode inside the actual
write transaction before replay or fresh mutation. Incoming API does not select
or alter owner mode. Ordinary reads still reject active purge.

After replay returns an exact receipt, failure to load progress or validate its
start anchor returns CommittedMutation carrying that in-hand receipt. No second
lookup reconstructs evidence. Failures before a receipt is available retain their
original error class. The existing unknown-child test still returns the reserved
CommitOutcomeUnknown identity and recovers correctly. Cancellation retains worker
ownership. Coordinating-agent and Curie bounded reviews of earlier source found
no blocker; final source inspection verified these combined boundaries on a9053ef.
Those source reviews are distinct from the runtime results below.

## Actual validation

All runs were isolated Docker, network none, two CPUs/4GiB, Rust1.94.1,
CARGO_BUILD_JOBS2/RUST_TEST_THREADS2, private source-specific targets. Final source
and genuine BEFORE artifacts were read-only mounted, with tracked file SHA256
checks inside the container before and after each final run. No personal data,
credentials, daemon or socket mounts. See the [evidence README](../evidence/native-request-context/README.md)
and [final manifest](../evidence/native-request-context/final-manifest.json).

| Check on a9053ef | Result |
| --- | --- |
| Complete context-focused filter, with genuine BEFORE fixtures/old executable | 10 passed,24.37s |
| Unknown-child recovery regression | 1 passed,1.90s |
| API1/API2 known-receipt damage/missing-progress and prelookup missing-receipt matrix | 1 passed,8.83s; also included in the ten-test filter |
| Full default-feature library suite | 1103 passed,715.68s |
| Actual lifecycle/text migration CLI tests | Both passed,2.59s/5.13s |
| Formatting | Passed |
| Default all-target Clippy | Exit0,869 library/929 test warnings,840 duplicates |
| All-feature test compilation (`--no-run`) and all-target/all-feature Clippy | Exit0,890 library/950 test warnings,862 duplicates; one main-binary warning, duplicated for test target |

Four existing doctests remain ignored. The optional bench feature adds command/
module code but no tests or conditional core branches on this source. Its targets
were compiled/linted without repeating the full runtime suite. No local
all-feature full-runtime execution is claimed. All logs and executable hashes
are retained; no Rust rerun is required for this documentation/evidence tail.

## Preserved failures

- Genuine old runtime parent3fa91d4, exporter-only1b4eb26: all four force/CAS
  combinations authored, completed, exported/imported and forked by the old
  binary. PASS28.02s;180-file artifact manifest retained.
- Deliberately unanchored proposed API1 candidate f79614e: stripping all three
  start copies before any child incorrectly passed preflight, so the negative
  test failed1.37s. This is a candidate bug, not a published API1 exploit.
- Frozen cb6a452 full:1100 passed/1 failed516.61s. New coordinator ordinary control
  read blocked active purge before replay; exact diagnostic574850d retained.
- a4acd5f owner correction restored unknown-child classification (PASS1.57s),
  then the actual known-receipt regression failed1.10s on bare InvalidControl
  after successful lookup.9e70229 supplies the narrow receipt wrapper, validated
  by the final a9053ef source. Both REDs and the earlier full failure remain.
- Earlier test compilation and read-only fixture-mount failures have separate
  source/log/binary manifests. Sealed BEFORE originals were never modified.

No source-root switching reused an executable. The negative candidate used a
separate package-cleaned target. Heavy jobs drained before the explicit quiet
window; only source/docs changed during HOLD. Validation resumed after explicit
END22:14UTC, preserving the earlier a4 source pointer/binary before rebuilding.

## Actual changed sources and remaining consumers

Production changes: storage/request_context.rs and mod.rs; native_purge.rs;
metadata/purge.rs; metadata/native_catalog.rs; native_policy.rs; and the actual
derived_completion/native_purge.rs coordinator. Regressions are in the catalog,
policy and purge tests, including new purge_context_tests.rs and
native_purge_tests/request_context.rs. Exact paths are in final-manifest.json.

This is an actual PR121 consumer slice, not ordinary startup/public activation.
Huygens owns real versioned transport projection. Curie owns compound batch
consumption and graph/link producers; Pascal owns independent lifecycle/exposure/
manual propagation. Standalone writers do not inherit this codec for coordination.
The [startup plan](native-startup-admission-plan.md) keeps prepared fixture-tested
adapters before ordinary API1-native admission, then public API2 plus actual A2
selection. The [legacy API2 inventory](legacy-api2-writer-inventory.md) separately
records real legacy restrictions and catalog caller gaps without blanket widening.

Next assigned slice: Feynman consumes actual124 plus this validated parent to
route the three native catalog/policy receipt calls through the owned allocator,
preserving typed purge starts and124 batch/child reservations. Halley's prepared
8c3494b proof draft/merge review is input, not executed evidence. Halley now owns
integration; A6 constructor/control/checkpoint work transfers to a helper. No
moving WIP parents, second engine, implicit migration, profile blessing,
atomic-batch claim or arbitrary power-loss claim is introduced here.
