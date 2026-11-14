# Explicit coherent similarity: frozen implementation plan

Actual parents: published PR133 `27a6cd4c6fc89f19dcd8964c20ec3fbb8b24c36a`
and PR132 `5864c09050e532454c97ba8e73e00e7ba1670fb4`, joined at
`821e991a9f459a45fa2d000e066b8bbe73288cc3`. No moving startup/writer branch is
required for this owned-fixture consumer. Hume owns selection and returned-ID
exposure; Huygens owns strict wire and later activation; Pascal reviews observer
scope/origin integration. Frozen parent branches remain untouched.

Three actual baseline probes failed as intended: phase filtering after k+1
returned an empty page despite an eligible lower match; similarity returned a
superseded predecessor; and the existing MCP false same-namespace flag was
ignored. The last fixture has unknown legacy profiles and proves ignored input
only, not profile compatibility. Input manifest
`9f525548a615b35596570d0a1e2ae86725f6bab7aff27902a488ce2048176c07`,
binary `851f82d0e4840ac6d2a667a5101a0006cedcd0d2779b749a3740cd4db85895eb`.
All three expected assertion failures and pre/post-identical source are retained
under `.validation/baseline-1`. This is not final candidate validation.

Add an internal explicit similarity selection path with independent semantics,
same-namespace/all-compatible scope, filter, original API number and requested
access. Reuse actual canonical ReadView, derived leases and one sampled clock.
The reference vector is the canonical by-ID source; source namespace is resolved
by ID and an explicit name remains an assertion. No provider call or profile
inference is needed for comparing stored vectors. All-compatible requires a
genuine Bound source profile and compares canonical target bindings; unknown,
different-profile and different-dimension namespaces are excluded with reasons.
Invalid canonical bindings fail rather than becoming an exclusion excuse.

Project each searched namespace with its own pinned policy at the shared time.
Apply phase and all supplied hard filters before similarity allowance; score with
the existing exact dot-product convention. V2 current resolves same-scope chains
and applies the maximum admitted own/inherited cosine once; history keeps eligible
stored versions independently. Do not call fused relevance scoring or apply RIF.
Use the existing shared chain-examination/hop bounds and deterministic UUID ties,
with returned-only version/provenance diagnostics and honest scan/omission counts.
Any exhaustive canonical/vector enumeration is explicitly a cost, not a bounded
allocation or performance-promotion claim.

Preserve V1 cosine scoring and stored-version behavior, while correcting its
canonical phase-before-cap defect in the existing shared similarity helper.
The source-only decision and pending validation are recorded in
`similarity-v1-eligibility-decision.md`. Correct the
existing explicit legacy `sameNamespace:false` path by routing it through the
shared compatible-scope consumer with V1 raw-cosine/history semantics. No implicit
broad scope, unknown-profile compatibility, public V2 route or startup activation.

Hydrate only final selected IDs at the pinned generation, preserving each
namespace. A private selection proof retains source identity/profile and returned
target bindings. Under the existing observation worker's ownership/lock order,
re-read the current source and all target scopes before the first mutation.
Resolve explicit per-target-namespace assertions; never pass a foreign source to
a same-namespace capture or silently use scope=None. Reuse existing ordered
namespace groups, per-namespace policies, one sampled time, original API identity,
known partial receipts and full publication fallback. No second observer or
atomic-multi-namespace claim. False/empty selection invokes no unused observer.

Required candidate checks: real Bound matching/mismatch/unbound scopes; eligible
refill; current/history and filtered successor; deterministic ties and chain
failure/work limits; source assertion/visibility failure before exposure;
returned-only IDs and per-namespace classification/time/policy/origin; later-group
commit failure retaining earlier receipts; false byte identity; actual legacy
false-flag consumer; V1 eligible top-k against an exhaustive oracle; and unchanged
V1 cosine scores/stored-version behavior on already eligible inputs. Freeze new source,
use a separate candidate target, and run offline Rust tests before claiming a fix.
