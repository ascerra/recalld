# PR112 final test-only main172 alignment

Exact source `ca9a1e53a5aa16a1317a51d79414e05bcf2c888b` normally merges sealed PR112d99 with actual main172 `19bd383a75cdb15283b4c15a865dfcb2a4b6e0a1`. Its only five source changes versus d99 are test files byte-identical to main172. Production/Cargo/examples/scripts are unchanged. Huygens supplied an exact source ACK; no runtime guard changed.

All7 stages PASS: **38 unique affected Rust tests/38 executions**, fmt and allfeature/alltargetClippy with warnings. Groups: query_feedback_chronology_, similar_v, adopted_public_, native_record_context_, api1_public_. This is **not a new full/public run**. [d99 bounded proof](../pr112-d99d69ce/REPORT.md) remains146focused/public9/54true; [31c full proof](../pr112-31c8c7d6/REPORT.md) remains1665library+10CLI+15examples/3ignored. The [31c driver-attachment correction](../pr112-31c8c7d6-driver-correction/REPORT.md) and original failed runs remain separately attributed.

Pinned offline Rust1.94.1 Docker,2CPU,actual6GiB/privateHOME/data/onecache; ownpackageclean only after prior binaries sealed. All502 input hashes unchanged before/after; executed binaries rehashed before reuse. Peak6442450944bytes,maxevents3157,OOM0/kill0. No4GiB/performance claim. Container retired. Exact final-headCI/currentmainCI stillgate merge; no ordinarynative/public2 activation.
