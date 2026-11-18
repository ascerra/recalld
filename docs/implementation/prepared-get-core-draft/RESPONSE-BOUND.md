# Prepared get response bound — direct consumer proof draft

This covers the actual single-point observer and compact GetReply/MCP DTO. It
is not a general AccessRecorder contract, capture-RSS bound, arbitrary outer MCP
request-ID guarantee, or certification of the still-unjoined private get RPC.
No receipt, returned text, labels, event, or numeric outcome is truncated.

## Before exposure

`GetReply::fits` counts the actual serialized `ToolCallResult`. Both structured
and text copies derive from the same `serde_json::Value`: a finite f32 may get a
longer f64 spelling in Value, so counting the typed JSON alone is insufficient.
The existing payload, scope and pre-exposure readGeneration are counted exactly.
A false/terminal response adds1024bytes for bounded wrapper overhead. A true
Full/Summary reserves65536bytes before calling the observer. Oversized payloads
are rejected before any exposure; the final whole-frame check remains.

The actual point recorder supplies exactly one distinct UUID/direct event in
one namespace. Native captured intent must match that singleton, source and
namespace; legacy uses the same singleton ordered exposure writer. Therefore
there is at most one exposure receipt, with the same full receipt appearing in
`receipt` and the one-element `observationReceipts`. Publication errors retain
those receipts; storage exception strings are not copied into the get wire.
General multi-namespace queries/foreign peer evidence require their own bound.

## Scalar/key algebra (compact Value JSON bytes)

Object size is2 + (field count−1 commas) + sum(key length+3+value bound).
An array adds2 brackets and count−1 commas. Bounds include quotes where needed:
all signed/unsigned integer scalars20; finite float spelling32; fixed enum,
UUID or quoted generation38; bool5; null4; each SHA byte3. Two fixed32-byte SHA
arrays each occupy129bytes. Nullable timestamp/event branches are filled in the
typed specimen; feedback must be null for exposure, not a variable payload.

| Actual type | Field count | Bound |
| --- | ---: | ---: |
| LifecycleFields | 10 | 452 |
| LifecycleState, including LifecycleFields | 11 | 824 |
| AccessEvent | 2 | 80 |
| LifecycleChange, two states/two hashes/one event | 7 | 2151 |
| GenerationRef | 2 | 102 |
| NativeLifecycleWrite, singleton records, no feedback | 8 | 2546 |
| Tagged lifecycle-exposure outcome | 2 | 2607 |
| WriteReceipt | 6 | 2837 |

Thus4096bounds the entire native receipt, including all keys. Legacy singleton
exposure is smaller. The test traverses the real typed Value, checks finite
scalar encodings and computes this conservative key/scalar bound; adding a
field or enum beyond the allowance must update/review this derivation. The
max-width native transition passes the actual transition validator; it is a
serializer specimen with synthetic hash arrays, NOT claimed committed history.

Completion replaces/adds fixed fields only: success status, at most one fixed
warning, an error with fixed code/message and UUID, receipt-container keys, and
four fixed readiness components. Existing namespace/readG/payload were counted
before exposure. Even including all alternatives together,8192raw bytes bounds
these fixed fields: four readiness objects each under128, readiness header
under256, error fields under2560 (including a256-byte diagnostic pointer encoded
as at most1538bytes), warning under256, and all envelope/container keys under512.
Point errors currently have no variable diagnostic field except input parsing.
An unknown input key producing a pointer longer than256bytes now returns the
same error code at the whole-request pointer with a fixed message, before work.
No payload/receipt is removed from an already committed response to fit a frame.

Compact JSON contains no unescaped control bytes. Encoding that JSON as one
text string costs at most2N+2; its structured copy costsN. Both receipt aliases
plus completion therefore fit3×(2×4096+8192)+1024=50176bytes, below65536. The1024
covers MCP constant container fields, inserted commas/quotes and the existing
numeric-u64 daemon wrapper (plus four framing bytes); tests serialize the actual
wrapper at u64::MAX. This is source framing coverage, not a live get-RPC test.

## Required concrete checks and limits

Typed maximal native/legacy receipts, actual Value-normalized float spellings,
escaped/control/Unicode payloads, exact/±1 admission thresholds, fixed unknown
failure with full receipt aliases and bounded input diagnostics are included in
this draft. Direct actual MCP/REST malformed-key controls assert unchanged
store bytes. Actual engine frame-boundary controls assert no event on reject,
one complete receipt/readiness on admitted true mode, and no writes in false
mode. Earlier actual native commit/unknown and publication-failure controls
remain separately attributed; serializer examples do not replace them.

Live direct MCP/REST use8MiB. Caller-supplied tiny internal test limits may be
smaller than even the fixed error frame (the earlier128byte reject probe was
never a transport claim). The later real private get RPC must pass its actual
negotiated F, reject unsupported capacity before work, and retain malformed-peer
receivedEvidence without assigning it current-operation commitment. Do not
claim the direct reserve establishes those R1 consumer requirements. The corrected bound module passed3 checks at893; full1718library/10CLI/15examples
then passed on that source. See the core-final seal for the retained31db test-only
failure and exact attribution. RPC remains pending; no O1 READY claim.
