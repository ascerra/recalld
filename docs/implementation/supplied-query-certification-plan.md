# Supplied-query certification consumer

Approved scope, 2026-11-15. This is an actual prepared consumer, not a helper PR
or public API2/native-startup activation.

## Exact parents and ownership

- PR175 `7283a700c4eca5f8d1d8907b33c867b1d1e3a6bb`: owned vector certification.
- PR151 `2b2560516e1e8e4548c9511f296a72862110caa9`: actual prepared MCP/REST
  query and similarity consumers, strict selectors and profile assertions.
- Feynman's direct175 helper `3f76d77e47715499b1ec9180514fec8af2578018`:
  extracts `CanonicalReadView::check_native_vector_namespace` from the existing
  generated-query admission. Frozen follow-up
  `794506f6cec23d949fb600000690017ae085dcf4` exposes the existing validated
  native-owner bit through `is_native_owned()`. Both helper commits are consumed
  unchanged. No helper PR, PR149, PR176, compaction, or moving parent.

Feynman owns `storage/metadata/publication.rs` and confirmed the helper interface
and accessor on 2026-11-15. Huygens owns the NamespaceResolver trait/delegate,
pipeline supplied-vector admission, prepared query/similarity consumers and tests.
Hume confirmed no active `similar_selection.rs` edits and explicitly acknowledged
the source and every compatible donor guard, with existing ranking, history,
scope, coverage, hydration and observation behavior preserved. Request his
source review again at the frozen implementation. Constructors remain unchanged
until this consumer proves the required boundary. PR176 stays frozen with Halley.

## Implementation boundary

Use actual format authority from the captured canonical read view, not the
native-query feature flag, namespace fields, vector width, or provider defaults.
Native supplied query vectors must pass the existing exact-generation, canonical
namespace/full-config and owned certification check before vector access.
The generated path retains its additional active-provider identity check.
Do not infer caller-vector provenance or change existing dimension, numeric,
normalization or explicit prepared-profile validation.

Ordinary queries remain single namespace. `allCompatible` belongs only to the
existing similarity operation: check its source before copying the source vector,
then every already-profile-compatible donor namespace before reading its vectors.
All checks use that operation's one pinned CanonicalReadView and generation.
Unknown/different-profile/dimension exclusions remain unchanged. A compatible
but uncertified namespace produces a whole typed error before exposure, never
silent omission, partial success, or inflated coverage. Do not gate unrelated
namespaces or add a global certification loop. Preserve current/history,
supersession, filters-before-caps, ranking/ties, final revalidation/hydration and
returned-ID observation. Metadata-only and nonsemantic FTS work stay provider-free.
The existing legacy compatibility lane and public API2 gate remain unchanged.

## Evidence and validation

Before the fix, add and execute intended-rejection tests through the actual
internal query boundary and prepared MCP/REST adapters on valid native stores
with uncertified vectors. Preserve source/executable identities and actual RED
results. These prepared transports do not claim ordinary public API2 activation.
Do not fabricate authored schema rows, bypass startup validation, or call policy
adoption alone certification. The positive fixture uses explicit re-embedding
and its resulting certification remedy through the actual operator/storage path.

Cover the feature-flag-false bypass, source scope/config identity and generation,
certified source plus uncertified compatible donor, incompatible/unknown excluded
donors, and certified success. Preserve false-mode canonical records, receipts,
history and generation. Assert no provider calls for supplied-vector and
metadata-only cases; retain generated active-profile and numeric/profile negatives.
Use existing error projections and strict full-known-receipt handling unchanged.

Run focused RED/GREEN first, then one frozen full suite and appropriate fmt/clippy
on immutable read-only source in offline Rust1.94.1 Docker, 2CPU/4GiB/two build jobs.
Reuse the single owned Cargo cache/target, hash executed binaries before switching
source roots and clean only the owned package target. Retain failed evidence;
release completed raw roots centrally after publication and recorded results.

## Implemented boundary and review

The live `StorageNamespaceResolver` also reports the actual storage format. It
cannot grant certification itself: its strict check retains the rejecting default,
so an unpinned native resolver cannot use the legacy compatibility lane. Owned
coherent reads replace it with the captured canonical resolver. Custom/legacy
resolvers retain their existing default; they grant no native certification.

Hume reviewed the production four-file diff on 2026-11-15, SHA256
`85ea70fc606d2c9bf0493bebe54b35157c926b702f3da0b83f5668033a1e2187`:
source checks precede `get_vector`; donor checks precede searched coverage,
records and vectors; errors escape before observation; pinned hydration remains.
This is a source review, not independent execution evidence.

The similarity positive fixture now explicitly re-embeds retained documents via
the real remedy, then performs real native Full→Summary→Ghost transitions. Its
synthetic document provider preserves the declared geometry without reading old
vector bytes. The original ranking, scope, terminal/history and phase assertions
remain. An additional fixture certifies only the source and proves a compatible
uncertified donor fails the whole allCompatible operation. Policy adoption is
only lifecycle setup; it does not certify vectors or repair historical output.

The Rust internal unpinned `search_with_semantics` entry now rejects native
supplied-vector requests, even on a certified store: it cannot bind certification
to a captured generation. Callers must use the owned pinned coherent entry.
There is no silent fallback. Direct nonsemantic metadata selection remains valid,
as do legacy-disk compatibility calls. The inherited native writer/reader fixture
now asserts that deliberate rejection while retaining its original pinned positive
result, labels, generation and false-mode byte checks. This is an internal API
behavior correction, not public API2 activation.

Feynman independently source-reviewed the same four-file diff and acknowledged
actual format authority, rejecting unpinned default, unchanged shared helper,
source/donor scope and generation checks, and unchanged generated-provider checks.
Neither owner source review claims independent execution.
