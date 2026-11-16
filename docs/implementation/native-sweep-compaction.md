# Native sweep compaction consumer

Actual parents: PR14522760a2 and PR13524da9206, joined without broader history.
Pascal explicitly released this consumer to Feynman; Pascal retains public API1
feedback/forget and source review. PR145 stays frozen for Halley.

Reuse the existing native phase producer and PR145 receipt-aware completion.
After a successful pass strips full text or deletes records, select one physical
text generation and await publication. Manual lifecycle origin remains1; startup,
timer and internal trigger remain2; physical compactText stays internal1. No
second scheduler, system initializer, new proof codec or implicit native gate.
A known/unknown prior phase failure must stop fresh compaction. Preserve its
receipt/operation ID and no new generation/reservations; after selection preserve
its exact new receipt. Successful zero-removal selections still report success.

Prove actual manual and autonomous callers, internal original hash, strict reopen,
unchanged lifecycle state under relocation and actual readiness. Existing final
record/policy/graph guards and sampled clock remain. Test-only baseline first
exposes the currently skipped native physical work. Existing partial-publication
regression remains a required negative. Runtime/global inherited-config audit is
separately owned and remains source-only while this consumer validates.
