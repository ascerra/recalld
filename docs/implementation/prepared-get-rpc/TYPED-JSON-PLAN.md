# Get-only typed JSON carrier — owner review required before runtime edits

Current runtime is ecfe45643cee9205428912d00e12f2eabd051e49, private child of
41ba core +200a R1. Focus33PASS/1FAIL/1historical ignored in49.39s: actual native
RPC, receipt faults, ping/reconnect and cancellation passed; the extreme scalar
frame failed. This plan does not transfer those counts to a future carrier.

Exact dependency diagnostic shows f32 bits00000001 serializing `1e-45`, then
Value parsing/serialization alternating with `1.0000000000000001e-45`. Both
preserve f32 bits, but neither passes the existing exact f64 Number spelling
predicate. MAX/near-one/normal controls were stable. The failed encoding
specimen is not claimed canonical persisted history or a published-store defect.

## Choice and scope

Prefer a private get wrapper `{wireVersion:1,httpStatus,replyJson:string}`.
`replyJson` is emitted directly from the typed GetReply serializer, without a
Value hop. Decode it directly into GetReply, require exact typed serialization
bytes, then apply every existing ID/scope/phase/receipt/readiness predicate.
The wrapper remains unpublished; public routing, DTO/receipt storage codecs,
request hashes, global serde features and R1 query/pairs behavior are unchanged.

Per-field bits would require a second field mapping for MemoryResponse and every
receipt variant. It would duplicate type authority and be easier to omit when
fields evolve. A string carrying the complete typed JSON retains the existing
serializer and validator. Do not wrap the old Value decoder in a string: both
whole GetReply decoding and receipt revalidation must avoid Value-before-typed.

First prove actual GetReply flatten + tagged native receipt aliases roundtrip
subnormal/adjacent/MAX/signed zero bit patterns, with valid lifecycle-domain
states and canonical typed bytes. Synthetic encoding specimens remain separately
labeled from actual owned producer/history tests. Probe unknown/duplicate/missing
nested fields, invalid numbers, f32 overflow and fractional integer fields.
No runtime revision until the exact proposal/probe receives owner/queue ACK.

## Literal prospective hunks

- `daemon/prepared_get.rs`: string wrapper emitter; direct typed byte-canonical
  decoder; borrowed named raw-fragment evidence extraction; unchanged semantic
  checks and typed postcommit failure retention. Add native response phase and
  created-at/captured-pre-observation last-access binding to the validated captured-before state
  if source review confirms projection does not change them. Never require
  generation adjacency or substitute a legacy captured-policy witness.
- `daemon/prepared_query/shape.rs`: factor only the existing compact/duplicate/
  depth/node traversal so get can use a separate bounded typed-JSON numeric
  mode. Existing `check` retains its exact canonical Number predicate. No query
  tolerance, dependency feature, retry-until-stable encoding or history change.
- `daemon/prepared_query/decode.rs`: reuse the same receipt type/base identity
  validator with a direct raw canonical JSON entry; leave receipt(Value) and
  query typed(Value) semantics unchanged. All supported outcome kinds can be
  retained as UNVERIFIED evidence, including foreign BatchReserved; only the
  existing singleton exposure kinds can be attributed to current get.
- Get tests/encoding derivation only. No N1 catalog/create/runtime edits and no
  new parent. Other shared hunks remain under existing owner reservations.

## Before-acquisition and before-observation budgets

1. Outer frame retains R1's exact compact scalar/structure preflight at get F.
   It contains only fixed integers and a JSON string, not unstable float leaves.
2. Before allocating the typed inner reply or acquiring any receipt, borrowed
   inner traversal applies the same compact/duplicate/depth/node bounds. Charge
   the complete inner byte length plus `max(0,32-raw_number_length)` per numeric
   token (integers included conservatively). Reject if that conservative normalization
   bound exceeds get F. This is a numeric byte bound, not approximate equality.
   The visitor may allocate bounded keys/sets; no zero-allocation or RSS claim.
3. Direct typed serialization must equal the entire inner string byte-for-byte.
   Semantic validation then binds actual returned UUID/source/access/sample,
   terminal visibility, full receipt aliases and exact current-G readiness.
4. Named untrusted receipt fragments use direct typed/raw canonical validation
   before conversion to the existing receivedEvidence Value carrier. Preserve
   their original parsed Value at that boundary; never serialize the decoded
   receipt to manufacture replacement evidence, recursively promote evidence,
   or attribute a foreign receipt to current commitment/readiness.
5. For admitted inner bytes, prospective Value encoding is bounded by F, so
   structured+escaped compact text costs at most3F. The existing fixed error
   overhead plus one-point numeric/completion reserve is deducted in choosing F.
   Re-derive and test the quoted inner wrapper too: core pre-exposure fits counts
   full MCP payload+65536/1024 reserve; the private string wrapper is separately
   measured at maximum numeric request ID. Final dispatch refuses payload only,
   retaining exact known/unknown evidence in typed result.

Oversized or structurally invalid inner strings remain sent-unattributed before
any typed evidence is acquired. This is not permission to discard an acquired
receipt because later serialization is inconvenient. Test real native extreme
producer data, received evidence from different supported receipt kinds, F-1/F/F+1,
maximum IDs, escaped/Unicode payloads, unknown fields/variants and failure frames.
Existing actual cancellation/no-retry/clock/origin/receipt tests must still pass.

The new direct typed test is a diagnostic prerequisite, not implementation or a
helper-only shipping PR. Full validation/current-head CI/ordinary-route closure
remain open. No changes to personal stores or benchmark experiments.

## Concrete accounting proposed for the owner gate

The actual enabled dependency features are `default, raw_value, std` for both
serde_json build fingerprints (012/SERDE-FEATURES.json), not arbitrary_precision
or float_roundtrip. Cargo.lock pins serde_json1.0.150 and zmij1.0.21. The former
ser.rs write_f32/write_f64 and number.rs Display use zmij::Buffer::format_finite;
zmij's BUFFER_SIZE and Buffer storage are24bytes. Integer u64/i64 forms use at
most20bytes. Thus32 bounds every finite Number emission for this pinned build,
not merely the sampled values. Feature/version changes require reviewing this
proof. Nonfinite JSON is rejected; typed f32 overflow cannot pass exact-byte
roundtrip. Checked addition rejects charge overflow, never wraps or saturates.

All numeric leaves count, including every SHA-array integer, timestamps and
counters. A normalized native receipt bound must be recomputed from the actual
shape with32 per number,38 per UUID/quoted-generation/enum, exact keys/commas and
arrays. The old4096 serializer bound is NOT reused for normalized charge. The
new diagnostic requires the resulting bound<=8192 per singleton receipt.

Let B be the actual pre-observation MCP bytes counted by core.fits. False point
success has exactly10 numeric leaves (source-shape assertion), so its complete
inner normalized charge is <=B+320. The quoted private wrapper adds fewer than
1024 bytes relative to counted MCP; fixed missing/input/terminal errors are
checked separately and do not carry observation receipts. They cannot create
postcommit evidence loss because they never observe. True point success uses
B+65536<=F. Added inner normalized completion costs at most
2*8192(receipt aliases)+8192(fixed readiness/error/warnings)+320=24896<65536.
Its quoted private completion uses raw receipt bounds4096, hence at most
2*(2*4096+8192)+1024=33792<65536. Therefore pre-admission implies BOTH inner
charge<=F and quoted frame<=F, including full known/unknown receipt errors;
this is stronger than merely fitting the raw private frame. The prototype test
measures the actual final shapes at the core's last accepted escaped-text size.

For arbitrary supported foreign evidence, singleton8192 is not assumed. Before
any typed acquisition, the entire raw inner string is charged with all numeric
leaves as above; normalized charge<=F bounds subsequent Value bytes. Structured
plus escaped text is <=3*F, with fixed remote-error/u64-frame overhead counted
from actual emitted forms and extra64KiB reserved for known typed scalar growth.
The diagnostic covers native/foreign BatchReserved and a large synthetic legacy
exposure receipt, all access/sent error combinations, and independent exact±1
inner-charge and quoted-frame ceilings. Those are encoding specimens, not
canonical commits or a new namespace/history authority. Actual runtime checks
must implement the same count and then repeat these boundaries on real RPC.

The test-only analyzer may allocate RawValue pointer maps; it proves accounting,
not production pre-allocation behavior. Runtime will reuse the bounded visitor,
retaining its key/depth/node/duplicate controls in both modes. Literal release
still requires the diagnostic results and exact owner ACK. No global query or
stored codec has changed.

## Fixed completion allowance: source authority

The8192 allowance excludes receipts, payload, namespace and readGeneration (already
counted in B). It is not inferred from one serialized specimen. Actual
`search/get_wire.rs` emits one-point success with `Readiness::from(storage::Readiness)`
or `GetReply::failure`; `query_response.rs::failure` never echoes storage errors.
The concrete shape bounds, including32 for EVERY numeric token, are:

- Four `DerivedReadiness` objects: each <=128 bytes (only names graph/vectors/fts/
  entities, quoted supported generation <=21, boolean, three fixed keys).
- Readiness header <=256 bytes: UUID38, quoted generation21, pendingWork32,
  ready5 and fixed keys/brackets. observedWork/pendingWorkScope are absent in
  this conversion, not a generic readiness-observation bound.
- Error <=2560 bytes: fixed category/code/commitState/message, optional UUID38,
  false retryable and fixed keys. Get codes <=64 and messages <=256 serialized
  bytes; the only caller-derived input pointer is capped256 UTF8bytes, hence
  <=1538 quoted JSON bytes even if every byte needs six-byte escaping. It is
  prework only. No observation error source text is exposed.
- At most one local failure warning <=256 bytes (the fixed READINESS_UNKNOWN
  message); fixed status plus added envelope/receipt-array/container keys <=512.

Sum4*128+256+2560+256+512=4096<8192. These per-field limits come from the actual
constructors above, not an arbitrary implementer of SearchPipeline. Remote
UNVERIFIED_RECEIVED_EVIDENCE has its separately measured fixed outer overhead.
Changes to those constructors, readiness conversion, fields or emitted constants
require updating the proof; test guards below check the emitted key sets and
allowances in addition to end-to-end sizes. Neither this derivation nor the
diagnostic permits dropping acquired evidence after an oversized decode.

## Implementation and local validation seal

The owner/queue released these hunks after1d9/013 passed. Runtime1dd implements
them; source0e3338 (test-only tails) passed1764library/10CLI/15examples, fmt,
include-format, Clippy with warnings and doctests (4ignored). See
[final RPC evidence](../../evidence/prepared-get/rpc-final/REPORT.md) for exact
source/executable/failed-run attribution. Current-main composition/CI and ordinary
public point-consumer activation are separate remaining gates.
