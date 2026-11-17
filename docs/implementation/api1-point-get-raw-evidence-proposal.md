# API1 get: raw evidence proposal after actual final-wire failure

**Source-only; not approved production behavior.** Runtime remains2925. Test sourcefb3bb725 and [sealed3PASS/2FAIL report](../evidence/api1-point-get/budget-wire/REPORT.md) establish that preserving an intermediate Value does not preserve it through actual final frame parsing. The exact current private O1 decoder and global serde/ToolCallResult contracts must not be silently changed to hide that failure.

## Smallest proposed wire boundary

The new API1 failure has no contractVersion/data. Keep typed current receipt/observation aliases, scope, readGeneration, readiness, error/category/message/field/retryable/commitState, readyfalse, original unknown operation ID and warnings as already reviewed. Change only **unverified received evidence** for this new API1 route to:

```json
{
  "error": "COMMIT_OUTCOME_UNKNOWN",
  "commitState": "unknown",
  "ready": false,
  "receivedEvidence": {
    "receiptsJson": ["<exact admitted original receipt JSON fragment>"]
  },
  "warnings": ["UNVERIFIED_RECEIVED_EVIDENCE", "READINESS_UNKNOWN"]
}
```

This is an illustrative subset, not a complete failure example or an actual receipt. Each string is copied from the original admitted raw named receipt fragment, **not serialized from a parsed Value or reconstructed typed receipt**. It remains unverified evidence, never current commitment/readiness. No operation ID is manufactured from it. Receipt version/hash/storage bytes do not change; this is a proposed new API1 error-carrier field, requiring Huygens' wire agreement.

Why this survives the existing final emitter: both public text and structured content contain strings. The final ToolCallResult Value/frame parse cannot numerically normalize bytes inside those strings. Parsing a returned raw string once with the same parser reproduces the original acquired Value, including the demonstrated cycle cases, because the input bytes are identical. Known current receipt objects retain their separate complete typed-byte/bit checks. Do not require an impossible byte identity between two differently typed public representations or pretend the old Map<Value> alone retains raw provenance.

## Actual acquisition seam, no second receipt authority

- The **new API1 remote method** owns a bounded raw-evidence companion alongside its GetReply result. Direct actual QueryEngine get has no received remote evidence. Public constructors must not accept arbitrary Values and invent raw provenance.
- Before any typed receipt/evidence acquisition, apply the existing bounded outer/inner structural checks, duplicate/depth/node limits and all-numeric charge. Reuse the existing `prepared_query::decode::receipt_raw` validator for admitted named `receipt`/`observationReceipts` fragments. Retain the original fragment bytes after validation. Preserve order and existing duplicate handling; no recursion into a peer's receivedEvidence field.
- Reuse the existing whole-get typed validator for valid native results, with only a reviewed visibility/helper seam if necessary. Existing `get_contract_v1` wire, GetReply serialization and O1 remote caller remain unchanged. No global serde feature, field-by-field float-bit codec, retry-until-stable normalization or replacement receipt parser.
- The new API1 failed remote result carries those raw fragments to its API1 projection. Project `receiptsJson` as ordinary strings through ToolCallResult; the actual bytes are immutable in that path. The current receivedEvidence Value can be checked against parsing each retained raw fragment, but must not supply the serialized provenance.
- An authored to_value specimen without original raw acquisition is not a valid production input to this companion. Keep020's authored failure as a diagnostic of the rejected representation. Do not forge a raw fragment to make it green. The actual canonical-raw acquisition test must pass strict original-Value equality after final framing and one explicit raw-fragment decode.

## Budget change that must be proved, not assumed

Raw JSON as a string adds another escape layer. The old3F allowance is not applicable to the new opaque string list. Conservatively, received raw JSON strings may cost2F in the structured form and4F inside the text form: derive a new6F+fixed-overhead+point-reserve bound, including array punctuation, actual warning/error keys and the new private tag. Recompute the prospective allowance from the actual emitter; do not merely change a multiplier constant and claim proof. A uniform smaller incoming allowance is the simplest proposal; disclose its conservative capacity tradeoff. Do not silently keep the larger allowance then discard evidence after acquiring it.

The pre-observation check must still cover **all three actual encodings**: private normalized inner, private tagged outer, and API1 success pretty text+structured output with actual timezone. Keep a source-derived reserve for full current known/unknown completion and enforce the producer shape/diagnostic limits. Repeat last-admitted payload, independent ±1, signed-zero, numeric extrema and large raw-foreign-evidence tests. Use the actual framed writer/reader and actual remote route when implemented; no in-memory-only transport claim.

Arbitrary public JSON-RPC ID overhead remains an actual activation gate until its budget reaches admission. This carrier change alone does not establish whole public conformance or native startup readiness.

## Capable legacy route: preserve types before bounded projection

At2925, ObservationError has typed receipt(s)/unknown ID but an unconstrained source message; Display embeds both in a PartiallyApplied string. A hard bound on that string is not proved. Do not parse or truncate it after conversion.

For the new capable API1 route, retain typed observation failure **inside the existing legacy capture→observe method**, before `map_search_error`/`BridgeError` string conversion. One UUID/source scope yields at most the actual singleton exposure receipt; preserve every returned receipt and original unknown ID, and assert that source invariant rather than quietly dropping extras. Emit a fixed bounded diagnostic selected from the typed cause/commit state, with receipts in typed fields. Preserve captured scope/readG if available; never reconstruct them after the write. Recompute the full typed legacy error bound independently from arbitrary bridge strings.

An optional emitter check at that same old capture→observe boundary refuses oversized successful payloads before exposure. The old alias wrapper continues to map the result through the historical errors and preserves old clock/payload/lifecycle semantics; new native owners refuse those aliases before work. No extra observer, policy lookup, accepted root or completion registry. Valid old-peer cap absence remains the exact old compatibility lane before send; it is not evidence of disk format or full receipt conformance. Observing exchanges must never retry after uncertain transmission.

## Review and next actual proof

Huygens: agree or reject the exact `receiptsJson` API1 error field/parse semantics and final ToolCallResult route. Halley: exact helper visibility/client/server/legacy typed-error hunk ownership after the parent gate. Feynman: raw receipt validation/provenance and capacity accounting; no parallel codec. A3: legacy singleton/cause semantics remain unchanged by the new typed boundary.

Only after this bounded design review: implement the test-only raw companion/actual-framing diagnostic, then obtain literal production release. Keep017 real3RED,018/019/020 diagnostic failures and their source/executable provenance. No additional parent or public2 activation is proposed.

The reviewed test-only diagnostic now passes five focused checks at738b; [exact report and provenance](../evidence/api1-point-get/raw-carrier/REPORT.md). This does not release runtime hunks or transfer those counts to ordinary consumer code.
