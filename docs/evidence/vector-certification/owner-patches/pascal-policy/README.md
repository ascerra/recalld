# Four policy/replay fixture corrections (source-only)

Exact source: Feynman candidate024; all four baseline SHA256 values verified in MANIFEST.json before copying. Owner froze only these agreed hunks. The isolated before/after trees contain four test files, no runtime/helper source or Cargo cache.

- exposure_consumer_tests: per-namespace failure adopts only the first successful namespace through the real helper; all bindings initially None and the second terminal binding remains None.
- public_api1_tests: configured multiplier2 positive closes all owners, actually adopts2, reopens with the same clock/policy and rebuilds before feedback. Existing original1 hash/receipt assertions remain.
- wire_tests: the legacy-policy positive explicitly adopts its separately selected schema destination. Existing adapter construction is extracted as configured_public_adapters for the multiplier2 setup; no additional test module or runtime interface.
- request_context_tests: genuine old API2 ZIP uses actual insert_generated replay before fresh capture; no adoption/certification. Preserve original receipt/file/G checks, contract1 conflict, clock0 and add provider0/None assertions. Ignored historic fixture generator is unchanged.

Corrections are the reviewed PR166 test hunks (3f3ca05), not a merge of that branch or its runtime. No shared fixture or certification helper was modified. The ZIP was read only to confirm its existing d9f61e87... hash.

Validation performed here: source review, SHA256 comparison, and git apply --check against the isolated before tree. No Rust compile/test/fmt execution; prior166 evidence is not execution against this certification candidate. Feynman owns integration and focused validation. Check all before hashes before applying; the patch and expected after hashes are in MANIFEST.json. No publication or commit was made.
