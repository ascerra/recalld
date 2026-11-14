# Native single supplied profile-ID boundary

This is a minimal child of PR118 (`f910c00`). It does not add PR111, a new
profile registry, a provider-derived descriptor selector, or a new hash domain.
Huygens owns public wire/batch preparation; this child owns the single producer's
typed profile-ID boundary.

`McpStorageAdapter::store_native_single_by_profile_id(ctx, input, profile_id,
policy, parent)` requires `input.insert.embedding_profile == None`. A supplied
vector requires `Some(profile_id)`; generated vectors require `None`. The
original vector bits, optional caller fields, arrays and duplicate-check intent
remain unchanged. This method is internal; existing public dispatch is gated.

The metadata resolver first validates scope/lineage and looks up the existing
key. Existing single hashes already encode the supplied descriptor's derived
profile ID, not descriptor bytes. An additive internal hash helper accepts that
same ID and emits the same old encoding. Matching replay reconstructs the full
request hash using the retained receipt's numeric defaults, profile commitment
and settings, then returns the exact receipt before current namespace, policy,
provider, settings or clock resolution. Changed profile IDs or any other
meaningful request field conflict. Replay still precedes expected-generation
CAS. It does not fabricate a descriptor to make old replay fit a new API.

Only a fresh supplied-vector request reads the live canonical namespace binding.
`NamespaceConfig::decode` and schema/binding validation check the descriptor,
dimension and descriptor-derived profile ID. A well-formed but different ID returns typed `StorageError::EmbeddingProfileMismatch`
for the wire owner to map to `EMBEDDING_PROFILE_MISMATCH`/409. Malformed IDs and
invalid vector shapes remain invalid input. The requested ID must match;
`UnknownLegacy` cannot become Known from width, configured provider or caller
assertion. The validated canonical descriptor is then supplied to the existing
private insertion pipeline, which rechecks the binding and all other final
transaction constraints. Provider-generated vectors retain the original runtime
profile check. Publication failures retain the full receipt through the same
owner and advisory enrichment is unchanged.

## Retired-scope boundary

The helper consumes an explicit immutable namespace ID in `MutationContext`.
Matching replay does not read the live namespace or need a full descriptor after
purge. It does require admitted, validated retained receipt history. Actual
native purge/reused-name startup and consumer integration are not ancestors of
this minimal child and are not claimed tested here. That join must use the
existing validated retired binding/proof path if it needs name assertions or
other retired scope data. Per CONTRACTS lines 34-38, an ambiguous name-only
retry must fail; it must not resolve to a newly created namespace incarnation.
No private purge parser or artificial PR111 dependency is introduced solely for
a selector that matching single replay does not need.

Batch preparation remains separate: matching reservation replay must use its
validated retained `NativeBatchDefaults.embedding_binding`; fresh resolution
uses the canonical binding. Original caller profile-ID/vector intent remains
its hash authority. Neither path may query the provider before matching replay.

Validation includes a genuine BEFORE fixture made by the PR118 descriptor-input
producer plus a test-only exporter, then byte-preserving replay through the new
ID entry point. See [evidence](evidence/native-single-profile/REPORT.md).
