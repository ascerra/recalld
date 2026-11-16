# PR65 main172 composition compile failure

Exact source `91eac8ca8b189e311746689873fe2ad9a5673212` failed its first Rust focused stage with five E0061 errors in native_catalog_adapter_tests.rs: newly integrated fixtures call NamespaceRegistryAdapter::new without the provider argument introduced by65. No Rust tests/full/public episodes ran. Format and the three Python stages passed. Source stayed immutable; this is an integration compile failure, not a runtime result.

A narrow test-only correction is under Huygens source review: bind the same existing provider Arc to each constructor, including one shared UnknownProvider Arc for the unavailable-profile case. Assertions and production code stay unchanged. Corrected source requires its own full/focused/public validation and freshheadCI; earlierfa1full1510/8CLI/15examples remains separately attributed. Actual offline2CPU6GiB/privatecache; container retired. Failed raw root remains protected.
