# Namespace partitions in the existing derived vector owner

This consumer is based on PR1783361ddb2, not a broad runtime integration. It changes
the existing shared vector value to `VectorIndexOwner`, which contains either the
fixed-width compatibility index or canonical namespace partitions of the unchanged
FlatVectorIndex kernel. There is still one engine, vector lock, publication registry
and canonical GenerationRef. No persisted format, profile, receipt or canonical
record is changed by selecting this derived layout.

Native `Recalld::with_owned_storage` constructs partitions from the owned canonical
namespace catalog. The publisher creates fresh partitions from that catalog at its
existing guarded rebuild point, loads all retained physical vectors, and advances
the one vector watermark only with the other actual required views. Namespace IDs
and widths never come from the active provider or query. Same-width/different-profile
namespaces remain separate; partition membership grants no semantic permission.

The uniform compatibility layout retains the old fixed-width behavior and failure
checks. It cannot make mismatched vectors ready by skipping them: full publication
still fails on a wrong-width row. Native production construction does not select it.
Existing test fixtures using uniform indexes still exercise their original deliberate
wrong-width publication failures. This is not a native admission escape.

Search delegates to the same Flat kernel after mandatory native namespace/width
validation; a scope/filter mismatch errors even at zero cap. Eligible IDs cannot
broaden scope. UUID locations and total count update only after the Flat operation
succeeds; cross-namespace duplicate IDs and namespace-changing metadata updates error.
Native partitions are rebuilt on catalog change, including purge; empty partitions
are valid while absent partitions error. The one actual5namespace diagnostic proves
mixed4/5-width physical coverage and bits, then real native purge and rebuild.

## Public Rust compatibility changes

The shared handle now contains `VectorIndexOwner` in Recalld's vector getter,
DerivedCompletion, SharedVectorIndexRegistry live/pinned construction, API/MCP
adapters, graph autolink handle parameters and IndexHandles. Direct Rust callers
that construct these handles use `VectorIndexOwner::uniform(dim)` for legacy
compatibility. There is intentionally no global dimensions() or save() pretending
that a heterogeneous owner is uniform. Standalone `FlatVectorIndex`, its VectorIndex
trait, SIMD implementation, score bits, UUID ties, filtering and CHVI-v1 load/save
remain byte-for-byte unchanged. Kernel-only callers need no migration.

Native partition construction is crate-private and uses the canonical owner. No
public fixture loader or second native engine is added. Tests/examples that pass
shared handles receive mechanical type changes; unrelated kernel tests stay Flat.
Future152 coverage integration must report the actual partition examined count,
not the old global count or result heap length. That leaf is not imported here.

## Legacy graph/advisory boundary

`perform_autolink[_with_work]` receives the new shared handle, then requires its
uniform kernel before any vector search. A partitioned handle returns typed
`AutoLinkError::VectorSearch`; cap0 is covered by an actual no-mutation test. The
legacy duplicate advisory's actual API/MCP call sites similarly request the uniform
kernel and return typed unavailable errors rather than empty native results.

The old autolink callers are API `post_creation_links_with_work`, MCP's legacy
store body, and the LoCoMo legacy ingestion body. Their existing legacy nonfatal
handling is unchanged; this leaf does not claim to redesign those protocols. Native
single-store requests use the actual accepted native producer instead of those
legacy post-write paths. Curie owns the certified same-insert selection and standalone
graph consumer. Its future139 join must retain `require_legacy_owner` in legacy
wrappers **and** this uniform fence; native RelationshipGraphAdapter selection stays
GraphRequest::Automatic → canonical native producer → generation/certification check.
No requested native linking may be silently rerouted through legacy advisory handling.
Curie's source ACK confirms the disjoint hunks, not a composed runtime proof.

## Admission remains separate

Both private constructor profile loops remain unchanged. The startup diagnostic
continues to receive `Init { step: native_profile, ... }`; its new positive test
asserts that rejection, not successful startup. Only the independent publisher
changes from actual RED to GREEN. Ordinary native startup and publicAPI2 remain
closed. Huygens's validated query179, Curie's producer/graph leaves and the finite
startup closure still require their actual integration; no WIP parent is imported.

See `heterogeneous-owner-hunks.md` for exact owner ACKs, BEFORE attribution and scope.
Final validation is reported separately in `../evidence/heterogeneous-vector-owner/REPORT.md`.
