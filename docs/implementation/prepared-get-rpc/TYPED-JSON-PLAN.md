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
  immutable-created/last-access binding to the validated captured-before state
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
