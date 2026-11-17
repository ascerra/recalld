# Private exact-pair scalar correction

Owner: Huygens, isolated `impl/remote-prepared-query`, actual parent184
`5d18075eea193849056ae88cb07f27843d1c7019`. Probe source
`9c3297b8e3142c59dae589c7a029780601fed054`; runtime still `63d3691a`.
Halley explicitly released the scan RPC/prepared-pair/tests/docs hunks and the
small same-connection client capability/server-ping addition below. The queue
and frozen branches remain untouched. Feynman and Hume source-approved this
exact plan before implementation; Halley also source-approved the transport
boundary. These are source reviews, not runtime validation or READY claims.

Run009 reached the final assertion after all cases and canonical-byte equality:
0 passed /1 failed. Initial cosine bits were4606822130811788905.

| Threshold | Caller bits | Actual server cursor bits | Result |
| --- | --- | --- | --- |
| Below |4606822130811788904|4606822130811788903|Remote unavailable; local cursor rejected|
| Equal |4606822130811788905|4606822130811788906|Server omitted the equal pair; Remote unavailable|
| Above |4606822130811788906|4606822130811788906|Remote first page and its returned cursor worked|

The response's first drift is `compact_value`: cosine bits8905 become8906
before the socket. This is distinct from run006's test expectation correction.
Raw009 source/log/executable and all three case diagnostics remain protected;
executable SHA256 `ecab20e81a60edbd91a2b4db5bd100743acbdeca3e70778f51978f64fd2b5020`.
Modeled request/response bytes are labeled separately from actual diagnostic
exchanges; the actual server threshold is recovered losslessly from its cursor.

## Proposed implementation

- Private alias `exact_pairs_bits_v1` and ping capability `exactPairsBitsV1`.
  The existing same-connection handshake stores this boolean; pair calls require
  it plus `preparedQueriesV1` and the full negotiated frame. No second handshake,
  fallback, automatic replay, or support for the obsolete private numeric alias.
  Missing capability refuses before sending; the server still checks frame and
  owner support before parsing/capture. Query/similar aliases remain unchanged.
- Replace private request `threshold` with `thresholdBits`, exactly16 lowercase
  hexadecimal characters encoding IEEE-754 bits. Reject missing/null/numeric,
  wrong width/case, extra fields, or simultaneous numeric threshold. Decode once
  using `u64::from_str_radix` and `f64::from_bits`, require finite0..1 and canonical
  positive zero, then feed the existing public request validator an in-memory
  Number without reparsing decimal JSON. Public input parsing is unchanged.
- Replace private pair `similarity` with `similarityBits`, the same16-character
  encoding. The typed private DTO rejects unknown fields. Decode once, require
  finite[-1,1], and retain all existing ID/order/count/threshold/cursor checks.
  The codec preserves cosine signed zero (the current kernel emits positive
  zero). No loose numeric fallback or tolerance. Restored numeric scores never
  enter a decimal serialize/parse equality check.
- The encoder builds a bounded page Value, replaces each numeric score using
  the original PairEdge bits, and emits only the bit field. Public PairEdge and
  ExactScanPage serialization remain numeric. The existing whole-frame counting
  check uses the new actual representation; overflowing pages fail whole.
- Original kernel, stored vectors, threshold comparison, cursor format/binding,
  receipts, public/v1 wire, and global serde configuration are unchanged.
  Ordinary public2 admission stays closed. No additional dependency parents.

Files: `src/serialization/scan.rs` RPC-only helpers;
`src/daemon/prepared_query.rs`, `prepared_query/pairs.rs`;
the existing capability fields in `client.rs`/`server.rs`; tests and docs.

## Required validation

Keep original009 assertions: exact local/Remote score bits, complete page values,
below/equal/above membership, actual returned-cursor continuation, and unchanged
canonical bytes. Add malformed encoding/nonfinite/range/conflicting-field peer
cases, missing capability/limited-frame prework refusal, same-connection no
fallback, actual full-frame bounds, and the native known-publication-failure
receipt control. Preserve all earlier failures. Focused GREEN precedes one frozen
full/lint/public conformance run; exact-head CI and integration remain separate.
