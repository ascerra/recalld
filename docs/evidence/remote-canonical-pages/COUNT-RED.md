# R2 malformed continuation totals

Feynman's source-only review found that prior-anchor and future-entry checks
were separate instead of additive. Reviewed WIP prepared_pages SHA256 was
`92073bf39b4f0625a13a66aec6c13a8da7b0ce63470c78534c98e2612f8112c0`;
serializer SHA256 `5b32846c66af96abc797035681d1ca64d666aa1c5c02fecdba6ef081a1cbfec4`;
backend SHA256 `7fee03fbe225d1f9c154eb88f1d57fb2a8f4ca2bd3df414cd129d505396e9bea`.
That review was not execution. The executed probe below used frozen commit
`f81e5ee7e6d372a49e6ea2fef15e5d4cf4e8f6a8` after formatting and test additions.

The actual owner produced each valid reply. A synthetic framed peer changed
only `total`: continued list3→2, continued namespace catalog3→2, first truncated
entity bucket3→1. IDs, G, scope, data, order, labels and cursor stayed intact.
The real client negotiated metadata capability with preparedQueries=false.
All three malformed pages were accepted. The final assertion compared
`[false,false,false]` with `[true,true,true]` after all same-connection pings and
canonical immutability checks, preserving evidence for every case. No fallback
or replay request was observed. This proves a decoder consistency defect, not
remote authenticity or a backend counting defect.

Run002: `prepared_pages_` **3 passed,1 failed**, exit101,1.19seconds; actual native
writer→Remote→MCP/HTTP Unicode/authoritative-empty/reopen control **1 passed**,
exit0,5.08seconds. Compilation105seconds. No full suite or final READY claim.
Runtime source/input count466, all hashes unchanged. Executable SHA256
`bfa5114011d8cf70711cf2cf94a57762cb6a397a14289b8c12c055b4bee278a8`,627757168bytes,
preserved before rebuilding. Failed log SHA256
`ae391b3afbc677d9649a2e9cb1decdf2bd92073cef766e7ca7d4a0a63ac6e10e`.

Offline Docker, same image/Rust1.94.1/2CPU/4GiB/jobs1 as BEFORE. Cgroup peak4GiB,
max24555,oom0,oom_kill0; reclaim pressure, not OOM. Container retired. Protected
root `/tmp/recalld-huygens-active-validation/canonical-pages-002-count-red`
retains source, command, logs, verification and executable. No raw deletion.

Correction: checked lower bound `returned + hasIncomingAnchor + hasMore`, per
bucket for tag catalogs. The earlier anchor proves at least one prior entry,
not its ordinal. An exhausted bucket keeps its old anchor and needs total≥1;
another bucket may advance. First complete page retains exact-total checks.
No backend/hash/cursor-format/label semantics change. GREEN remains pending.
