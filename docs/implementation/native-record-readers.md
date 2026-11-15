# Native record publication and existing readers

This child joins the actual native record core `6a70174` (PR79, runtime
`acaa285`) and canonical reader parent `eefc987` (PR69). Merge `d1d916e` preserves
both parents' contracts: native component proof and owned work tickets, scoped
forget under its final storage guard, legacy observation receipts and canonical
entity rebuilding. It does not import batch PR78, offline batch PR77, writer-guard
PR80, or unrelated integration history.

The existing `acknowledge_derived_effects_with_work(operation_id, parent)` selects
the native publication guard for an explicitly native owner. It checks the actual
owner's ticket and validates pending effects before consuming only canonically
applied effects. This checkpoint changes neither canonical records nor mutation
receipts/generation. An uncertain metadata commit makes the handle unavailable;
unresolved work cannot become a successful drain. General legacy writers remain
blocked on native owners.

The real `DerivedCompletion` coordinator still rebuilds graph, vectors, FTS and
canonical entity postings under its existing lock order, invalidates cache, and
acknowledges afterward. `complete_with_work(parent)` keeps its existing behavior.
The explicit crate-private `rebuild_all()` invokes the same implementation when
new views are constructed after reopen and the outbox is already empty. It does
not invent an outbox row or committed receipt. A rebuild error without a pending
operation remains an unavailable error, not a fabricated committed outcome.

The production-backed tests live in
`src/storage/metadata/native_records/reader_tests.rs`, using the parent module's
real schema/profile/lifecycle migrations and native writer. The fixture takes
`NativeLifecycleSession::engine`, the already owned native engine, through
`Fixture::from_open_engine`. That helper constructs empty views only. Actual
`DerivedCompletion` must publish before the real HTTP/MCP adapters can read.
There is no authored canonical row or separate legacy storage placeholder.

The live-record test covers HTTP and MCP get/list/similar, exact Unicode labels,
empty native labels despite a compatibility entity tag, and unchanged
`LegacyRetainedOnly` provenance. MCP get retains its existing wire shape without
a top-level entity alias; its canonical `labels` remain exact. The same production
pipeline also runs a filtered metadata-only query and supplied-vector query with
a provider that panics on any profile/dimension/embedding call. Every explicit
false-mode sequence leaves canonical files byte-identical. Clean-outbox reopen
uses the real rebuild path and repeats the transport checks. A subsequent forged
label is rejected by strict native startup without changing source files.

The forget test commits through the existing native transaction, forces a real
vector-index rebuild failure, verifies the receipt/outbox remain and reads stay
blocked, then retries publication. Terminal text/labels disappear from point
reads and lists, the vector is absent, another owner retains its vector, and
reopen preserves the tombstone. The ordinary observing route still rejects the
native owner without canonical mutation.

The original end-to-end test failed at the legacy-only acknowledgment guard
after native insertion committed. Its exact source, log and executable identity
are retained in the validation evidence. Intermediate test corrections retained
MCP get's response shape, the existing HTTP list route, and metadata-only's
explicit filter requirement. None required a runtime relaxation.

The joined full suite also exposed a missing typed namespace-error mapping for
scoped forget. The mutation adapter now preserves `NamespaceMismatch` and daemon
code `-32007`. Two fixture corrections retain the actual canonical contracts:
foreign entity postings have a real record owner when purge rebuilds them, and
observation receipt counts include the existing namespace-creation baseline.

This is a private native entry and existing reader composition, not public
contract-v2 activation. Ordinary schema-2 startup remains closed. The existing
effect-stamp admission does not establish a coherent whole-query ReadView or
generation watermark; Halley's actual registry consumer owns that later join.
Feynman owns additive native catalog/phase-loss/purge/batch proof extensions after
this freeze, preserving PR79's existing serialized insert/tombstone proofs.
No additional canonical writer is enabled here.
