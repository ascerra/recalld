# Native API1 store consumer

Approved next scope after PR151. This branch starts at published PR148
`508d98c65b0f824bbcc0290812c63e0dea426edd`; do not merge the query branch or
convenience startup/catalog ancestry. PR148 already supplies the actual original1
single/compound producer, profile validation and document_v1 generation recipe.
This plan is not an implementation or activation claim.

The current REST create handler and MCP store_memory_reserved path still invoke
legacy insertion. The private native_single wire handler constructs original2,
so calling it with an API1 request would lose original request identity. Use the
actual store_native_single_by_profile_id producer with MutationContext original1,
accepted ownership, canonical namespace binding, and current-owner publication.
Physical format comes from owned canonical control/admission, not namespace
width, schema-shaped fixture rows, API version or provider labels.

Implement the actual format-aware single-store public path first. REST retains
MemoryResponse and MCP retains StoredMemory successful shapes. Native labels use
the canonical UTF-8 writer rather than legacy tag merging. Legacy validation,
labels and no-profile supplied-vector behavior remain unchanged. REST gains the
approved additive embeddingProfileId: malformed400, native missing422, mismatched
binding409; an explicitly supplied legacy ID must be verified or rejected, never
ignored. Generated native vectors use the actual configured provider descriptor
and PR148's document recipe. MCP API1's historical ignored single embedding is
preserved. Unknown API1 fields do not acquire implicit version2 semantics.

Keep original receipts through known publication/hydration/worker failures.
Complete the actual accepted owner before observing readiness; do not add a
second outer ticket which self-drains. Native atomic links must not be followed
by the legacy post-creation link pass. Uncertain workers remain tracked and do
not become notCommitted. Budget and negotiated frame checks precede mutating
work; preserve full bounded receipts and existing successful API1 projection.

Pascal owns the in-flight lifecycle/client capability changes: api1_lifecycle
connection state, api1LifecycleV1 negotiation, reconnect reset, same-connection
nonretry call and feedback/forget aliases. Hold those hunks until his validated
handoff. Extend that one negotiation mechanism mechanically for store; never
add another handshake or retry a mutating new method via a legacy fallback.
Old-peer selection happens before any write. Curie owns mcp/batches.rs,
batch_adapters.rs and the new legacyAPI2 private4 preparation; do not edit those
files concurrently. Native API1 batch projection follows the existing PR148
original-context producer without selecting private4 or advertising it.

Required proof: actual REST/MCP/daemon old/new single-store routes in both
physical formats; native generated document recipe; explicit native vector
binding and caller/provider error distinction; canonical UTF-8 labels through
reopen; old successful shapes and ignored MCP single embedding; no provider,
UUID, staging or bytes on fresh invalid requests; original1 immutable receipts;
full known receipts after actual completion failure; actual small/unnegotiated
frame rejection before work and no mutating fallback. Public2 remains closed.
Ordinary startup consumes validated adapters later, without depending on public2
activation. Use the one protected private Cargo/target with source-specific
package cleaning after executable provenance capture; no per-run cache clone.
