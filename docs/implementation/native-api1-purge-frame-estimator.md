# Implemented purge admission inequality: source review candidate

This describes unvalidated implementation following the approved c6b5755d plan.
Candidate006 remains immutable. No full-suite or readiness claim.

## Wire bound

`mcp/native_purge_api1.rs::receipt_budget` serializes the existing DaemonResponse
error envelope with maximum u64 response ID, -32006, longest fixed code/state/scope,
all optional fields, a 36-byte UUID, a null receipt placeholder and 512 NUL diagnostic
bytes. NUL has the maximum JSON expansion per input byte. Let its serialized size
be E. Any compact serialized receipt of N bytes has no literal control byte. Its
embedding in error.message adds at most one escape byte per original byte, so the
complete frame payload is <= E + 2*N. Retaining the four placeholder null bytes
strictly overcounts. Admission permits N <= floor((peer_limit-E)/2), checked for
underflow. Actual full success envelope is separately bounded with the declared
64-byte name bound, maximum numeric counters/ID and the longer observed bool.
No framing limit is changed; the existing four-byte length prefix is outside the
payload limit. Local direct MCP receives no invented daemon response cap.

The code/state combinations used only for sizing need not be semantically valid;
they are never returned or treated as evidence. All actual projection validation,
matched-child precedence and unknown identity rules remain unchanged. Runtime
serializer inequality tests and actual complete frames remain required.

## Canonical receipt bound

`metadata/purge_budget.rs` uses serde_json::to_writer and a checked counting sink,
not a Vec containing a whole hypothetical receipt. The exact start outcome is
counted inside a WriteReceipt sizing envelope with maximum-width receipt IDs/G.
The deleting-child scaffold is counted with empty arrays, u64 maximum sequence,
u63 maximum previous/committed G and a full UUID parent. Cleanup and Complete
chunk=None scaffolds are separately serialized. These sizing objects are never
validated as real receipts, committed, exported or used as identities.

The deleting bound adds every target deletion witness with its retained numeric
access history, every incident edge, and a before+after source peer witness for
EVERY incident edge. This deliberately overcounts duplicate sources and includes
sources in the target namespace: a target may first survive as another chunk's
peer, then be deleted. Each item contributes its actual JSON length plus one comma;
empty scaffold already contains all array brackets/field names. For any permitted
chunk, each actual witness has a counted counterpart and no additional source can
appear while the target purge is active. Reducing counts/removing rows cannot
increase this sum. Repeated sources are counted without allocating a global set.

Only peer edge_count and record_generation may change between start and any child.
Both before and after sizing states use u16::MAX and Generation::MAX respectively,
covering every representable decimal width. Other record content/lifecycle/history,
profile IDs, edge payloads and vector references remain unchanged. The content
capture uses a 32-byte all-255 sizing text hash, whose decimal digits upper-bound
any actual digest; it never reads text or claims that hash as retained evidence.
Other content hashes are calculated from unchanged retained fields. Legacy-inferred
text commitment remains absent according to the existing capture function.

The [Huygens source audit](native-api1-purge-bound-source-audit.md), copied from
be4d6e4052e31d4c3a86bd328ff48c6d85452570 without importing ancestry, maps the writer
barriers. Fresh begin's final active-namespace helper excludes a second active
purge globally. Ordinary effects final TX uses the same helper; native catalog's
owner/publication guard excludes it without literally calling that TX helper.
Competing continuation of the accepted parent remains allowed. Actual races and
historical-route negatives are still required; source review alone is not proof.

## Memory, work, transaction and compatibility boundaries

Fresh native bounded calls only: at most 100,000 examinations (record, namespace,
edge and repeated endpoint lookups all count), 256 MiB cumulative encoded source
bytes, and each decoded value <= min(receipt_limit, 8 MiB). Root namespace size is
checked before decode. Overflow/cap/serialization failure refuses before any start
write or owned ID draw; a partial scan never authorizes mutation. Existing fresh
native record/force checks move into the bounded scan instead of running an
unbounded pre-scan. The unbounded existing route retains its prior checks.

Global metadata and forward edge tables are streamed using their existing redb
transactions. The admission path does not call load_all_edges or create a whole
edge Vec. Live heap is a bounded number of individually capped decoded values and
small sizing structures, not a full canonical snapshot; redb's existing pager
memory is not claimed to be bounded by this added counter. Repeated endpoint lookup
work counts toward the cap. This conservatively refuses some valid large unrelated
stores and high-degree namespaces; it is not a new disk/export/schema restriction.

Original request replay remains before fresh admission. Name-only public API1 has
no key and cannot enter the private Context replay branch. Active namespace reads
are unavailable; completed absent names are NotFound; recreated names resolve to
new IDs, never old receipts. No retained oversized receipt is rewritten/substituted.
The new refusal says operation too large with no retry/get_memory advice. A known
prior receipt can never be erased by projection into a fresh refusal.

Fresh schema/progress table mutations now occur only after admission. Transient
accepted tickets settle on known refusal; no claim of zero tickets ever admitted.
No new durable permit, registry, proof version, chunk selection, generation domain,
clock sample, provider request or ordinary startup activation.
