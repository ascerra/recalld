# API1 point-get handoff

- Branch `impl/api1-point-get`; actual parent `2925a7289055b27ffda2583a81b656442f347632`.
- Final tested source `33fd10af99cfe545bd19356e9e5979fceaab7ca3`; library executable SHA256 `0fd859566e2855e446470fbe7c991b455763c3399a7014389eeb4f62a2d00e48`. Final commit adds documentation/evidence only.
- Actual MCP/daemon API1 point routing, original public success shape, owning timezone, bounded borrowed raw-receipt admission, exact duplicate handling, full receipt/unknown identity, and no observing retransmission. O1 strict predicates/HTTP/D1 schemas unchanged.
- Runtime focus22PASS. Frozen full7c4:1831PASS/6FAIL/9ignored; all six were addressed by reviewed fixture updates. Tail25fe:8PASS/2FAIL; final33fd:the two corrected failures plus observer control3PASS. All ten selected cases have source-attributed passing runs. **No corrected-source local full-pass claim.**
- Full and final fmt/include-fmt/Clippy exit0 with warnings; docs0PASS/4ignored. All-targets build passed; later CLI/example tests were not executed after the library failure.
- [Report](REPORT.md) records every failure, correction and exact manifest. [Final proof](034-final-tail/SHA256.json), [failed full](031-frozen-full/SHA256.json), [tail failure](033-fixture-focus/SHA256.json).
- Integration: one reviewed current-parent join, affected checks, then required exact-head complete CI. Coordinate the final API1 failure/receivedEvidence shape with Huygens' grouped schemas. No public2/ordinary-native-startup/full-spec claim; arbitrary outer MCP-ID budget remains a separate boundary.
- No push/deploy/production merge. Freeze this branch once READY is sent to Halley. Central cleanup owns deletion;0hostbytes released. Protect `/tmp/recalld-curie-o1-build/{cargo,target,runs}` for current handoff, with one reusable cache and no new HOME archive; release exact completed raw roots after PR/results recording.
