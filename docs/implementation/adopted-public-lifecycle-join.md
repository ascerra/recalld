# Actual159+153 composition notes

Normal merge of159 b717939 (planning child858587) and153 a2a8866 only;
no other parent. Fourteen files conflicted. This records substantive choices,
not evidence of a runtime defect or executed tests.

- Keep159's `trigger_sweep_with_origin` and153 API1 lifecycle methods side by
  side. Manual sweep still delegates original1; no public-time change from162.
- Add153's lifecycle module exports, trait methods, Remote lifecycle methods and
  existing one-connection daemon test fixture. Keep159 native advisory module.
- Keep159's shared `direct_bridge` extraction; move153's actual `.with_query_engine`
  call into its `McpStorageAdapter`, preserving159's clock on both storage and
  namespace adapters. Do not reinstate a divergent main.rs constructor.
- Keep153's pinned reader refactor and rejecting observer, carrying159's
  `native_query_admission` into the pinned Self constructor. Remove the obsolete
  inline constructor. No new entry mode or public2 admission.
- Share the same existing DerivedCompletion handles between the observer and
  coherent reader in system.rs; retain159's `native_prefix` admission branch.
- `storage/control.rs` is byte-for-byte159 after resolving enum/match ordering:
  all typed inherited-policy proof and schema2 requirements retained, no wildcard.
- Retain159's MetadataStore owned ID source/receipt allocation and existing
  strict-open-derived `uses_native_lifecycle` getter; don't add a duplicate getter.
  Only153's actual legacy lifecycle metadata module is newly included there.
- Retain159's existing fixture helper naming/real adoption behavior. New consumer
  tests explicitly request unknown None without adoption; no blanket raw-schema
  rewrite or silent positive-fixture blessing.

Huygens source-reviewed pinned native admission, RejectExposure, direct bridge
and shared completion: no source blocker, not execution proof. Feynman independently source-reviewed the same named seams and unchanged typed
history proof: no blocker before validation freeze, no runtime claim. Any subsequent
compiled failures or required fixture adaptations will be recorded separately.

The merge includes inherited parent evidence files with trailing whitespace;
these remain immutable. Source diff checks must distinguish those preserved
historical logs/patches from new Rust changes.
