# Prepared Remote query consumer

Huygens owns this isolated child of
`5d18075eea193849056ae88cb07f27843d1c7019` (runtime `90daa782`). Halley
released the exact source and the narrow daemon/client/server/bridge/query-codec
hunks on 2026-11-16 under the user's standing authorization. The queue branch
remains frozen. Feynman acknowledged sole R1 client/server ownership and retains
N1 catalog/ordinary-startup implementation; no parallel connection edits.

Scope: the existing prepared query, similarity and exact-pair consumers over the
existing same-connection daemon protocol. No public2 activation, new registry,
ranking change, catalog/list dependency or automatic request replay. Preserve the
actual177 same-view source/donor certification. The local consumer's
select/budget/observe boundary remains authoritative.

The BEFORE at `a1ae5498b24506a9b8fe400e6ccc45c8c42978f1` compiled and reached
the intended final assertion: actual owned legacy prepared query200, Remote503.
Expected row, false record/G invariance and same-connection subsequent ping
passed. This is not a mutation or receipt-loss RED. Executable SHA256:
`eac3d6a07c1edf18f5b9440659b2956077b71c7fc04232404920cd8f43297e71`.
Raw/source/executable remain protected in
`/tmp/recalld-huygens-active-validation/remote-query-001-red`.
The 9198-file source manifest is a whole tracked-tree archive (including old
documentation/evidence), not 9198 build inputs. Future snapshots use only actual
Cargo/source/test/example inputs. The container exited101 and was removed;
cgroup peak4GiB/max25986, oom0/oom_kill0 records reclaim pressure, not OOM.

Validation is green at `09399ad1811396e17ad85e1b735c6d1334e75fb5`:
31 focused tests,1,706 library tests and10 CLI tests; formatter/Clippy exit0
with warnings. The final formal runner passed63 cases plus mutation negatives.
See [the sealed report](../evidence/remote-prepared-query/REPORT.md).
Exact-head CI/final integration remain separate; public2 is not activated.

BeforeSend/SentUnattributed/MatchedReply classification is additive for the new
consumer; existing API1 call/retry behavior stays unchanged. A partial socket
write or unattributable reply cannot establish no commitment. No fallback.

The malformed-response output policy has a reviewed schema boundary; final
consumer validation remains pending. Response capacity derives from the real
whole MCP text/structured wrapper, including
error/evidence cases; it must be enforced on the actual server before observation
and on raw received frames before typed evidence acquisition. Any accepted full
receipt must survive as evidence without claiming it belongs to the current
write. No arbitrary padding, silent truncation or receipt reference is approved.
Public capability advertisement waits for this proof and actual conformance.

The prepared-only daemon aliases are `query_contract_v1`,
`similar_contract_v1`, and `exact_pairs_bits_v1`; their private transport version does
not select application API1. The existing connection negotiates
`preparedQueriesV1` and full8MiB frames before an observing request is sent.
Exact pairs additionally require `exactPairsBitsV1` on that same connection.
The unpublished numeric `exact_pairs_v1` alias is unsupported; there is no
fallback. Private threshold/cosine fields carry16 lowercase hexadecimal IEEE-754
bits. Public numbers, the comparison kernel and cursor binding are unchanged.
See [the reviewed scalar plan](remote-exact-scalar-plan.md) for the actual
request/response drift that required this separate runtime correction.
Old peers receive no prepared request, and a sent request is never retried or
projected through a legacy fallback. Query parameters retain the resolved
namespace, selectors, access mode and supplied-vector bits. Namespace assertions
use the existing ASCII case-insensitive resolver contract; returned producer
display names are preserved.

The server uses the existing select/budget/observe boundary with a smaller
prepared response allowance. Let B be8MiB and E the largest **actual serialized**
fixed Remote-error ToolCallResult inside a daemon response with maximum-width
numeric ID, including the unverified-evidence variant. The raw incoming allowance
is F=floor((B-E)/3). Borrowed preflight rejects oversized frames, duplicate
object keys, noncanonical numeric spellings, depth above32 and objects above128
fields before constructing the owned response tree or acquiring typed receipts.
For admitted compact JSON, Value reserialization cannot grow scalar encodings;
its selected receipt subset occupies at most F bytes. Its escaped text copy
costs at most2F bytes, so complete evidence plus the measured fixed wrapper fits
B. This is a byte/decode-work bound, **not** a heap/RSS bound. Object keys and
later typed roundtrips allocate bounded intermediate values. Outer MCP string
IDs still require their own request-specific admission before public activation.

On malformed or unattributable replies, only `reply.receipt` and
`reply.observationReceipts` are inspected. Complete Rust `WriteReceipt` decoding
and exact canonical JSON roundtrip must succeed; the original decoded JSON is
retained. This rejects unknown kinds, ignored fields, numeric narrowing and
default-field insertion. It does not prove the receipt is authentic, persisted,
or associated with this query. `receivedEvidence.receipts` therefore appears
only on errors with `UNVERIFIED_RECEIVED_EVIDENCE`, without current receipt,
readiness, scope, readGeneration, operationId or success data. A sent observing
query remains `commitState: unknown`; a false read remains `notCommitted`.
Incoming nested receivedEvidence is not promoted or recursively searched.

The coordinator approved, under the user's standing authorization, a distinct
`receivedUnverifiedReceipt` schema: exactly six header fields, and an opaque
outcome object requiring a string kind. The schema deliberately does **not**
validate the outcome variant's complete structure or canonical semantics. Runtime
Rust decoding is stronger. Normal successful query/exposure schemas remain
separate and strict; missing observation groups cannot become successful true
reads. Successful receipts must cover the complete distinct returned exposure
set, with the actual source namespace and four known publication owners. The
decoder does not reconstruct ranking or infer ranked access kinds from a score;
only similarity and supplemental-only associative kinds are directly known.

Fixture classification: the parent-receipt peer test creates and reopens a real
legacy `BatchReserved` receipt, then places its unchanged bytes in an unrelated
malformed wrapper. The quote-heavy `BatchItemRejected` size fixture is separately
authored, Rust-codec-valid malicious peer data, not a valid canonical rejection
or evidence of normal producer payload limits. Exact F and F+1 cases test complete
retention versus rejection before typed acquisition. Native query/similar/scan
controls use the existing real migration, explicit policy adoption and actual
re-embedding fixture; they are not schema-authored rows or ordinary-startup proof.

Recorded development runs (not final validation):

- `d517d4af` /002: the original actual Remote boundary passed (1test). 452
  allowlisted build inputs verified unchanged; executable
  `e16fcd8225512903f4d7962530d81e429dd35f9f61b2442d9ccac75937479d75`.
- `8d36a189` /003: six source-review cases reached the intended RED: foreign row
  namespace accepted, missing/partial exposure accepted, fabricated readiness
  owner accepted, and valid mixed-case namespace rejected in false and true
  modes. Unmodified owned producer controls passed first. Executable
  `09934e208a6cc359c89e6ea5fbd7801e1db4ec5a09332ac47c2e14bc18ac8f81`
  is preserved with the failed source/log.
- `44290ce1` /004: compile-only E0308 in the new budget fixture (InsertResult
  passed where WriteReceipt was required); no behavioral tests executed.
- `ab953cdb` /005: 19 passed / 1 failed. Native rows retain the caller's valid
  mixed-case namespace display; the decoder incorrectly required an exact-case
  match. The correction preserves IDs and uses the existing ASCII name rule.
- `63d3691a` /006: 4 passed / 1 failed. The native metadata reply reached200;
  direct `to_value(local)` expected stability `3.7144999504089355`, while the
  actual framed response parsed as `3.714499950408935`. No canonical bytes or
  serializers were changed. Failed source/log/executable SHA256
  `479b94202041690ee2902d9555f51e9972e22125b1e971794788ceb96efb413c`
  remain protected.
- `76d545cb` /007: 5 passed / 1 failed. Feynman reviewed the exact test-only
  expected emitter path: typed private Reply -> `compact_value` -> framed
  DaemonResponse -> actual client Value parse. Full Value equality and canonical
  stability/vector f32-bit checks passed for metadata. The next fixture used
  unsupported `fts_only` and failed parsing before FTS or embedding execution.
  Executable `6b5943bdc53658fe0b27bd7dcf1a6cbb13a968cad928caff49f139ba17d910d1`
  and452 unchanged source inputs are preserved. Diagnostic artifacts distinguish
  local typed bytes, a synthetic expected frame with maximum-width request ID,
  and received final projected bytes; these are not raw network captures.
- `32d4de39` /008: 5 passed /1 failed after the test-only selector correction.
  The FTS case is now hybrid `embedding_plus_metadata`, explicit vector/profile
  and `ftsQuery`, with actual FTS/vector completed-channel assertions. Pure FTS
  is not exposed by this prepared parser; no selector or runtime was widened.
  All three query and four similar comparisons completed with exact expected
  wire Values/canonical f32 bits. The untouched pair equality then found local
  cosine `0.9599999979972839` versus Remote `0.959999997997284`. This is a
  separate f64 transport failure. Executable
  `2273da8e8e8b50a7b57ec4db0f11393ceea3c044a84c561e35f9ce911668409e`
  and452 unchanged inputs remain protected.
- `9c3297b8` /009: 0 passed /1 failed, the intended actual semantic RED.
  Below/equal thresholds changed bits at the server, the equal pair was omitted,
  and Remote/cursor controls failed. Above-threshold first page and its returned
  cursor passed. All cases reached unchanged canonical-byte verification before
  the final assertion. See the scalar plan for exact bits and executable hash.
- `09399ad1` /010: reviewed private scalar correction;24 prepared tests,1
  receipt-bound test,5 exact-pair tests and1 whole-frame test passed. All three
  threshold cases preserve caller/server bits and exact cosine bits, and continue
  using their actual Remote-returned cursor. The native known-publication-failure
  full receipt/reopen control passed. Maximum-count authored page serialization
  measured2,199,718 private-frame bytes against F2,795,934 (16,256 pairs); this is
  serializer evidence, separate from canonical native serving controls.
  The test's unnegotiated call proves RPC refusal and unchanged canonical bytes;
  refusal before capture is additionally established by dispatch source order,
  not by treating a read-only capture as a canonical write.
- `/011-final` uses the same unchanged09399 source and source-bound target;
  there is no source-root switch between010 and011. Its offline formal runner
  passed56 emitted input/output cases plus mutation negatives. Both runner and
  fixture mounts were read-only and their pre/post hashes matched. After the
  full run emitted additional fixtures, the same frozen runner passed63 cases
  plus mutation negatives. Full:1,706 library tests and10 CLI tests passed;
  3 existing library ignores and4 ignored doctests. Formatter andClippy exit0
  with warnings. The library executable hash is identical to focused010.

Validation uses one owned cache/target, immutable read-only source, offline Docker
Rust1.94.1, two CPUs/4GiB/jobs1. Source switches clean only the owned package.
Record source/executable hashes and failed-run attribution. Central cleanup owns
completed-run disposal; active and failed fixtures remain protected.
