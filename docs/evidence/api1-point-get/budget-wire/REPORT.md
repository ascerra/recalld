# API1 projection/frame diagnostic:3PASS/2FAIL; production remains blocked

Test-only source **`fb3bb725f0ab9923f8cd5cc8c348e1c9ee080a10`**, runtime parent2925 unchanged. Executable SHA256 **`6c11fb533e2106e0ba076f24dd4cc33b7b7e31d09804e6f842856c002c5171f3`**. Five diagnostics completed in6.46s:3PASS/2FAIL/0ignored. These results are not a production implementation/READY handoff.

## What passed

- New typed API1 failure prototype omits contractVersion/data and retains known receipt aliases, read/completion metadata and original unknown identity. Both text aliases exactly equal original typed receipt bytes; structured aliases round-trip to the complete canonical typed receipt. Signed-zero before/after strength also retains exact typed bytes/bits. Unlike018, this uses a typed/byte oracle rather than comparing incompatible Value constructors.
- Structured output now derives directly from the typed failure, preserving **in-memory** received-evidence Values. This corrects019's intermediate serialize/parse defect only.
- Actual API1 success rendering counts pretty text plus structured object and configured timezone. Measured73034bytes for UTC/Los Angeles and73038 for Kathmandu. Recomputed normalized receipt shape4933 and new fixed-completion proposal4992 feed an80KiB allowance (81920), separately from O1's allowance. Actual fixed error overhead1759 gives private allowance2768309.
- Independent normalized-inner and tagged-outer ±1 controls pass. At the last accepted671063-backslash payload, actual completed tagged frame2691761bytes and public success4029784bytes remain inside their respective ceilings. Full known/unknown point failure encodings fit the proposed reserve. The large15000-outcome foreign encoding specimen remains unverified and produces3421229public bytes from1635235raw bytes. These are encoding specimens, not invented canonical commits or universal source-limit enforcement.

## What still fails

Two new tests use an actual temporary Unix socket pair and the real `write_framed_message`/`read_framed_response` functions. One starts with an authored `to_value` receipt specimen; the other acquires evidence by parsing direct canonical typed receipt JSON, matching the raw acquisition mechanism rather than that authored Value path. Neither is mislabeled a persisted mutation.

Both preserve intermediate evidence equality, unknown/no-current-ID/no-current-receipt classification, and the full typed receipt's canonical bytes. **Both fail exact acquired-Value equality after final frame serialization and peer parsing.** `ToolCallResult.structured_content` is a Map of Value; converting to Value without an intermediate parse does not retain the original lexical numeric representation through the final serializer/parser. The complete failures are retained in focus.log. No tolerance, normalization retry or weakened evidence assertion was applied.

Consequently the prototype's in-memory correction is **not a transport fix**. Huygens/Halley must resolve the bounded evidence carrier/serialization contract before production release. Do not silently change global serde features, widen O1's private codec, reconstruct unverified evidence as current canonical authority, or claim complete public framing from the successful small/large specimens. Arbitrary outer MCP IDs also remain an actual remaining-budget/admission gate.

## Other source constraint: capable legacy failures

At2925, `decay/observation.rs::ObservationError` carries typed receipts plus an unconstrained diagnostic String; its Display serializes them together. `BridgeError::PartiallyApplied` stores that rendered string. There is no honest finite bound on the whole error merely because point receipts are small. The new capable-legacy route must retain typed observation failure before that conversion, use a bounded diagnostic projection without truncating embedded receipts, and check its success emitter inside the existing capture→observe path. Old aliases keep their historical mapping/clock/payload behavior, except the separately reviewed native-owner refusal. This is a source finding, not an executed legacy-overflow failure.

## Provenance and next gate

Build54.63s; actual build and test containers inspected before start, networknone,2CPU/4GiB, jobs1/tests2; one cache/target, same source root. Source and executable reverified unchanged after tests. Original017 real consumer failures,018 incorrect-oracle failure and019 prototype evidence-cycle failure remain separate with their own hashes. No full-suite rerun, runtime edit, parent push, personal store/daemon access, HOME archive or host cleanup occurred.

Raw `/tmp/recalld-curie-o1-build/runs/020-api1-budget-wire` and existing cache/evidence roots remain protected under central cleanup ownership. No publication/PR yet, no release inferred. Next action is bounded owner review of the actual wire-evidence constraint and capable-legacy typed-error seam, then literal production-hunk release and actual consumer/fault validation. The current branch still intentionally contains the genuine failing BEFORE tests; it is not shippable as a fix.
