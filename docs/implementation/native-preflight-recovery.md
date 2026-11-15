# Native startup validates repaired copies

Native startup previously rejected a real unclean process exit with redb
`RepairAborted` during strict preflight, before reaching its writable recovery
path. This occurs without batch operations: an actual native insertion exiting
before or after metadata commit reproduces it.

Only native preflight now permits redb repair of disposable metadata and edge
database copies. The existing complete validator still checks schema, lineage,
receipt proofs, pending effects, text, vectors and edge indexes before opening
the original databases for writing. Ordinary offline snapshot inspection and
import validation retain their existing repair-disabled behavior. This does not
apply effects during preflight or bypass malformed canonical-state rejection.

The engine already acquires stable store ownership and the directory lock before
preflight and holds both through writable recovery and engine lifetime. The
regressions use a test-only callback at the actual post-preflight boundary to
check both locks and compare every original file before writable open. A separate
process also verifies the owned store cannot be opened concurrently.

The immutable-rejection evidence covers crash startup without a pending restore
journal. Ownership acquisition can first complete an existing restore journal;
that separate recovery protocol is not claimed to leave all source bytes
unchanged on every subsequent startup failure.

## Regression coverage

- Actual native insertion exits with code 91 at `before_metadata_commit` and
  `after_metadata_commit`. Reopen recovers; retry commits once or replays the
  existing receipt respectively, with the expected generation and one parent
  edge. A second reopen passes strict schema inspection.
- Five actual migrated/native stores are corrupted in schema, pending effects,
  text, vector or edge state, then exited without closing redb. Copy repair
  succeeds, full validation rejects each malformed store, and original bytes
  remain identical. Test-only corruption is never used to create valid state.
- Each scenario proves at least one database requires redb repair. A valid
  allocator checkpoint can let the other database open without invoking repair;
  requiring both would incorrectly constrain redb behavior.
- The strict offline consumer rejects the unclean stores and leaves them intact.

The implementation is a minimal child of PR82, with no batch or catalog ancestry.
Ordinary schema-2 serving remains gated. The associated evidence report preserves
the failing source and executable identity, corrected logs, and the initial
test-construction failures separately from the reproduced runtime defect.
