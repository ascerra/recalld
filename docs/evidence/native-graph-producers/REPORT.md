# Actual standalone native graph producers

Status: validated locally, awaiting publication. Direct published PR121 parent
`3fa91d4e542ec8ed5519fe4c3b87e9e665043a40`; no PR106 batch parent, PR131 plain
record parent, compound codec, wire, broad validation or public activation parent.
The small actual completion seam is adapted from PR106 using the existing owner.

## Reproduced baseline

Test-only `d0de2ed57353349ed86d7dcf8ddb531f964dcb70` initially failed compilation:
the probe referenced the private `api::state` module. This is test scaffolding,
not a runtime failure. The corrected test-only source is
`f7f9304f977cef62c8372c729b4cda13bd8e7893`, on unchanged PR121 runtime.

All three actual probes then failed in3.35s:

- Original API1 `NativeLifecycleSession::put_graph` rejects the extra API2-only
  graph guard with `InvalidMutationContext`.
- The actual `RelationshipGraphAdapter` through its public trait returns
  Supersedes `Failed`, where the probe requested a persisted native edge.
- The actual public temporal producer returns `Storage("storage recovery required:
  operation does not participate in native lifecycle proof")`.

BEFORE library SHA256:
`ce748b6fec9e1007959016a7cd7c0521ee2c75dfa55137f42c36373ab251e42e`.
The failed compile and runtime logs remain separate. These are synthetic owned
native stores, initialized with actual migration/insertion; no authored native
canonical rows or personal daemon/data was used.

The API1 core probe becomes green with the narrow guard correction. The two
legacy-wrapper probes explain why those interfaces cannot acknowledge native
writes: their void/string projections cannot retain native receipts. Shipping
keeps their native rejection and tests the actual typed adapter replacements.
This is not a claim that three identical tests changed from failure to success.

## Actual consumers and identity

The real `RelationshipGraph` trait has `put_native_edges` and
`select_native_links`; `RelationshipGraphAdapter` implements both and production
`main` construction injects its real FTS handle, clock and lifecycle inputs.
Missing ownership fails before mutation. The completion object shares the
existing storage/graph/vector/FTS/entity/cache handles and publication registry;
there is no new registry or fabricated readiness state. Public results retain
`Option<WriteReceipt>`; internal effects remain private.

Caller-supplied ordered `NativeGraphInput` supports keyed replay before fresh
namespace/policy/endpoint capture or clock. Its existing hash, original API1|2,
CAS bits, signed weight bits and durable proof formats are unchanged. Automatic
semantic/entity/temporal requests reject keys before selection/clock. They are
fresh unkeyed operations, not a new retry contract for volatile selected edges.

Automatic selection scans canonical namespace records, verified canonical
vectors and canonical edges under the existing owner. It does not use stale
cache labels or claim a pinned derived selection. Namespace/live-phase filtering
and existing-edge exclusion precede the cap. Semantic vectors require a known
namespace profile and matching width/slot/content commitment. Entity rank is
shared canonical entity count then UUID; Jaccard is the edge weight. Temporal
rank is canonical creation-time distance then UUID. No provider is consulted.
A private ephemeral selection-generation field is checked inside the final
metadata transaction alongside the existing endpoint/policy guards, including
new unselected candidates. It does not rewrite caller expectedGeneration or add
any durable proof/hash/version field.

Accepted work precedes waiting/selection. The blocking worker owns publication
lease, graph guard, storage owner and child ticket through commit/application,
including cancellation of its async waiter. Lock order is publication, graph,
storage; no storage lock crosses an async graph wait. Completion then uses the
same owner to publish all four views at actual current generation, even for an
unkeyed no-change result. Known publication failures preserve the full receipt;
unknown commit identity and unresolved cancellation survive until reconciliation
or reopen. Process-exit checks do not establish power-loss durability.

Legacy void/string graph wrappers reject native owners before discovery or
mutation. Native creation must use the separate same-insert links proof and
bypass the whole legacy callback pipeline; this standalone writer cannot make
insert-then-autolink durable. Native LoCoMo/compound/API1 wire consumption remains
separate and needs the actual validated context/binding parents. This slice does
not activate ordinary native startup, public API2 or whole-store CAS.

## Validation

All tests use `recalld-ci:1.94`, Rust1.94.1, networknone,2CPUs/4GiB,2build jobs,
synthetic temporary stores and one private Cargo/target. No HOME/credentials or
sockets are mounted. BEFORE was an immutable archived source mount. The copied
private target was package-cleaned first; after changing the source mount back
to the worktree it was package-cleaned again before AFTER compilation. Every
source remains frozen while its validation runs.

- Initial AFTER: **13 passed**,20.23s. Source manifest/archive
  `source-focused-1` and executable hash are retained separately.
- Tested source `e4a47c3b6ef9a69c7665f4bc48059db7f4f980a4`: **14 passed**,23.82s,
  container exit0. Includes namespace/terminal filtering before caps and actual
  cancellation followed by reopen/replay. The subprocess entry intentionally
  returns without its control environment in the parent test runner.
- Actual proofs cover API1/API2 changed/no-change, exact stale-CAS replay and
  cross-contract conflict; real offline snapshot/export/import; all three
  automatic families; key rejection without clock/G/file changes; missing owner,
  scope/unsupported version; a new phantom candidate before the final TX;
  full receipt on vector-publication failure and replay repair; unknown commit
  identity through reopen; admitted descendants during barrier close; worker
  ownership after cancellation; and process exits before metadata, after
  metadata, after edges and after canonical acknowledgement.
- Source review then corrected entity ties to preserve existing shared-count/UUID
  ordering and replaced repeated all-edge scans with one adjacency set. A focused
  tie case is added, and the automatic-empty result additionally checks unchanged
  generation/clock. The follow-up was tested only after explicit cleanup END.
- Final frozen runtime/test source **`eb4959bffc4dc569faf851db369f188e98827709`**:
  **15 focused passed (27.51s), 1,108 library passed (601.60s), 2 CLI passed
  (2.89s and 1.55s)**. Four existing doctests are ignored. Format passed;
  all-feature/all-target Clippy passed in21.37s with warnings (947 lib-test,
  including860 duplicates). The complete Docker command exited0. No failure
  occurred after this final freeze, and no runtime change followed validation.
- All **257** tracked Rust/build/test inputs match the final source commit byte
  for byte. [SOURCE-MANIFEST.json](SOURCE-MANIFEST.json) retains their SHA256s;
  [VALIDATION.json](VALIDATION.json) pins image, source, test executables and log.
  Final library executable SHA256:
  `5f7ab3631208fb285fdb147b7b28c51d75d903b5d5b7934ed80742c2acc9d464`.
  Prior14-focus executable was hashed before rebuilding:
  `69a19ea77779f0a7d76a61fee6cfad6e5778b3a8f6327dfeeb9fe5e2751726df`.
- Hume reviewed the exact final worker/final-generation seam and found no new
  blocker. This is a bounded source review, separate from the runtime tests;
  it does not independently endorse every selection or startup path.

Final commands, run sequentially against frozen source using the existing private
Cargo/target, with no cache clone:

```sh
cargo fmt --all --check
cargo test --offline --all-features native_graph_producer_ -- --nocapture
cargo test --offline --all-features
cargo clippy --offline --all-features --all-targets
```

The final container mounted this worktree read-only at`/work`, private Cargo at
`/cargo`, private target at`/target`, and used `CARGO_BUILD_JOBS=2`,
`RUSTUP_TOOLCHAIN=1.94.1`, `CARGO_HOME=/cargo`, `CARGO_TARGET_DIR=/target`,
`--network none --cpus 2 --memory 4g`. Image identity is retained in VALIDATION.
To rerun BEFORE, use test-only source f7f9304 with the same focused filter. Use
an owned source-specific target or clean only this crate in an owned target
before switching source roots. Never reuse a previous executable as proof of
changed source. No `offline_eval` example is present in this ancestry; no
unrelated parent was added for that target.

## Retention and cleanup

The user's subsequent retention override permits disposing raw data once an
experiment has a resulting PR and recorded results. Do not create another HOME
raw archive solely to retain disposable files. This branch and its
single active cache remain protected through publication and any explicitly
registered future-input reuse;
compact source/results/provenance remain in the repository. The dedicated cleanup
owner alone deletes released roots, after checking active processes/mounts.


## Integration boundary and next owned work

Direct base is PR121 `impl/native-replay-consumers`; no additional prerequisite
is introduced. Preserve API1|2 context validation, native physical ownership,
namespace/policy/endpoint guards, and publication→graph→storage lock order.
The private selection-generation field is deliberately not serialized or hashed.
Creation callbacks remain legacy-only; native insertion must use its actual
same-insert proof. Ordinary native startup and public activation still require
remaining producer consumers, including native benchmark ingestion.

Next insertion/compound original-API work consumes the final validated insertion
wire, PR131 record consumer and PR134 retained-context codec, not PR138 or a broad
integration checkpoint. That separate slice must preserve historical native
absence=API2 versus legacy absence=API1, original reservation CAS, all-invalid
parent hash anchoring and final same-transaction child identity. Public routing
remains Huygens-owned. Typed supplied-vector classification is a separate logical
change; it must not turn arbitrary mutation/storage failures into caller422.
