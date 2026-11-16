# Existing Bound vectors: activation boundary audit

Source-only audit of PR148, `508d98c65b0f824bbcc0290812c63e0dea426edd`.
This is an activation blocker, not a codec change or a claim that the pending
API1 store consumer certifies historical vectors.

`model/schema.rs` stores the namespace descriptor/profile ID and binding origin
(`NativeV2`, `OperatorAttestation`, or `Reembedding`). Record schema stores vector
slot/allocation epoch/byte hash, labels, lifecycle and extensions.
`storage/native_records.rs::ContentProof` commits component hashes, vector bytes
and the advertised profile ID. Neither retains the actual input sent to the
provider, execution recipe version, or generated-versus-supplied vector origin.
`embedding/runtime.rs::GeneratedEmbedding` is ephemeral provider evidence; it
does not establish that a caller assembled the correct document recipe.

The genuine pre-correction `document-recipe-legacy` artifacts retained in PR148
use the same profile and proof shapes. Corrected writes do not add a durable
discriminator. Thus software version at open, generation, timestamps, API origin,
equal dimensions, profile equality and valid vector hashes cannot certify an
existing vector set. Label `NativeV2` provenance describes labels, not embeddings.
Old immutable receipts still need to validate and replay with their old meaning.

The existing `embedding/profile.rs::prepare_attestation` rejects every already
Bound namespace. Repeating that workflow is not a supported recertification
remedy, and a namespace's older attestation cannot certify subsequent writes.

Feynman owns the coordinated storage/admission proposal, with Curie reviewing
producer participation: explicit scoped AlreadyBound recertification or actual
re-embedding over the owned current vector set, binding subsequent admission to
that result and fencing writers that can invalidate it. No automatic blessing,
profile-ID replacement, history rehash, or invented lost provider input. A known
incorrect vector requires corrected generation, not an unsupported attestation.
Where retained content cannot reconstruct the original document, report that
limitation and require external evidence/content or a fresh destination.

Until that actual boundary is implemented and tested, ordinary native semantic
admission and public query activation remain blocked for uncertified existing
Bound vectors. Preserve offline inspection/export and old receipt compatibility;
the existing personal legacy-disk API1 unverified lane is unchanged. Required
future proof includes the genuine old producer artifact rejected for semantic
admission, successful explicit remedy, unchanged historical receipts, stale
vector-set rejection, and fresh generated/supplied writes maintaining the proof.
No new profile version or persisted codec is introduced by this audit.
