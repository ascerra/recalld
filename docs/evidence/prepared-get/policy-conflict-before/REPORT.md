# Actual policy-conflict mapping counterexample

Source d66fa354226e2eea6e923c9c26ff1b19b38c1b88; test executable aab671903e1cc82a8ab1917ac0096f84e92c263c97d4f9ef7db703ff6543dc5a. Runtime is unchanged from first-focus702; this adds a real policy-writer test.

0PASS/1FAIL in7.26s. Actual capture paused before observation after graph release; native apply_native_policy changed both policy_revision and target record_generation. Test confirmed retained exact policy receipt, no additional G/event, no get payload/exposure receipt, one original clock. The sole failing assertion was503 rather than409: the current generic observation mapper lost LifecycleConflict classification. No malformed raw history or fabricated policy-only/unchanged-token state was used.

Correction source-reviewed by A3/Pascal and wire/Huygens: internal serde-skipped cause, existing nested committed receipt extraction/unknown precedence unchanged, GetReply-only409 for typed conflict+notCommitted+empty receipts+no unknownID. Existing constructors and API1 external literal set None. Generic QueryReply mapping stays unchanged. Subsequent GREEN is not claimed by this counterexample.

Same private cache/target, no source-root switch; networknone2CPU4GiB, OOM0, testpeak116,637,696bytes. Logs/source/executable pins retained; no full suite or performance claim.
