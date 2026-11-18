# Explicit vector recipe remedy

This command regenerates one existing native namespace's retained vectors into a
fresh offline destination. It preserves the source, immutable profile identity,
original store lineage, historical receipts and payload provenance. A Bound
profile alone is not certification. Acknowledging the profile or upgrading the
binary cannot certify old vectors.

Use the source's inspected current store ID/generation and immutable namespace
ID/name/profile ID in a `ReembedVectors` JSON request. Generations are decimal
strings, not JSON numbers. For example (replace each source identity with its
actual inspected value):

```json
{
  "context": {
    "storeId": "11111111-1111-4111-8111-111111111111",
    "scope": {"kind": "namespace", "namespaceId": 1},
    "idempotencyKey": "explicit-vector-remedy-1",
    "expectedGeneration": "42",
    "contractVersion": 1
  },
  "source": {
    "storeId": "11111111-1111-4111-8111-111111111111",
    "generation": "42"
  },
  "namespaceId": 1,
  "namespaceName": "native",
  "profileId": "ep1:replace-with-the-existing-exact-profile-id"
}
```

The provider file contains `configuration` (the existing EmbeddingConfig),
`documentPrefix` and `queryPrefix`. Its effective provider/model/revision,
preprocessing prefixes, dimension and normalization must match that existing
profile. Set `configuration.cache_embeddings` to false. Request and provider
files each have a16KiB limit. They are explicit operator inputs; no inference of
provenance from credentials, model name or vector width occurs.

```sh
recalld-cli reembed-vectors \
  --source /offline/source \
  --destination /offline/certified \
  --request /offline/request.json \
  --provider /offline/provider.json
```

The source must be closed to other owners. One source owner spans immutable
inspection and provider work. Full and Summary rows use their actual retained
components; absent stripped text and LegacyRetainedOnly labels remain explicit.
Ghost rows retaining a vector but lacking an available document reject. This
slice does not synthesize lost input, change a profile ID, or certify arbitrary
supplied old bytes. Fresh supplied writes instead carry their existing explicit
caller profile assertion; fresh generated writes must carry the private seal
from the actual provider/document/raw-output path.

On success stdout contains the actual receipt and `readiness: reopen_required`.
It does not announce a derived-readiness barrier, complete a pending batch, or
enable ordinary native/API2 serving. Keep the original request for retries.
Matching committed destination/private-stage replay happens before reading
provider configuration, consulting source CAS or calling a provider again.
A known committed publication error returns the full receipt with
`recovery_required`; an unknown commit returns its actual allocated operation ID.

A process exit before the cutover transaction can leave an unpublished private
`<destination>.recalld-vector-certification` directory without a matching receipt.
Retry rejects it for inspection rather than deleting or repairing it. The source
is unchanged; another fresh destination can be chosen. After canonical commit,
retry publishes/replays the exact receipt without repeating provider work.
No power-loss guarantee is inferred from process-exit tests.

## Existing pending native batches

The explicit remedy can preserve strictly validated native2/3 pending manifests.
It does not execute missing child payloads or discard reserved identities. Each
pending scope must still have its original policy or exactly the validated
certification-only change. Ordinary backup/export/import/restore continue to
reject unresolved work. Unapplied effects, unfinished purge, unsupported versions
and other policy changes remain errors before provider/staging work.

For pending A/B/A work, remedy A to a fresh destination, then remedy B from that
destination using its newly inspected full source CAS. The original request may
resume A after its remedy; B remains pending until its own remedy. After both,
resubmit the original full batch and key: completed prefixes replay and remaining
children retain their original IDs/hashes. Same-namespace repeated cutover is
unsupported. Certification cannot rebind a retired/reused name or restore missing
input. Completion remains the actual ordered runner's responsibility.

## Bounds and compatibility

Certification inspection/reconstruction is bounded to100k rows and256MiB vector
payloads; proof payloads16MiB, retained history1million rows/512MiB, and copied
control databases512MiB. Format2 adds100k namespace/record and256MiB combined
record/payload and aggregate rounded vector-layout bounds before staging.
These new reconstruction limits do not alter ordinary uncertified format1 limits.
The private pending witness has an additional16MiB incremental heap budget,
including construction overlap and temporary namespace/hash encoding. It is not
a total bootstrap/process-memory bound. Owned queries/inserts add no full
certification/history scan; strict offline validation and owned reopen do scan.

Certified logical export requires format2 even when only retired certificates
remain. Placement hints reconstruct current locations only; final typed history,
root and exact f32 bits confer validity. Old binaries reject the required feature
and format. None serialization preserves old format1 bytes. Restore rejects an
older uncertified backup combined with a retained certified target before a
reconcile write. A separate old historical restore remains uncertified.

Actual results and failed candidates are in
[the evidence report](../evidence/vector-certification/REPORT.md). Later query,
constructor and physical-compaction consumer joins are listed there; this
operator path does not claim they are complete.

## Rust API change

`GeneratedEmbedding` now contains a private execution seal. External struct
literals are no longer supported; use the existing provider generation path.
Its public vector/profile fields remain compatibility payload, and modifying
either invalidates the seal. The crate-private `unverified` constructor is not
certification evidence and cannot authorize a generated native write. This is an
intentional Rust construction-API break, with actual producers/tests/examples
validated together; it does not change old serialized receipt bytes.
