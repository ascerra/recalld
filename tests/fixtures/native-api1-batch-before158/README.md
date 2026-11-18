# Genuine private original1 timestamp rejection

Produced by the recorded158 production source plus the test-only exporter in
`before-probe.rs.txt`, using valid migrated native fixtures. The private native3
Some(original1) producer finalized the timestamp item as INVALID_INPUT. This is
not evidence of successful prior public API1 batch acceptance. Both separate
public-route BEFORE tests failed.

`manifest.json` retains the original request/outcome/progress and artifact path.
`store.zip` preserves all six closed-store files byte for byte; extraction hashes
were verified against the source. PROVENANCE.json records source/executable and
file hashes. The regression extracts into disposable scratch before opening,
checks unchanged history on replay and rejects changed timestamp under the key.
Source/raw artifacts may be centrally cleaned after publication/results recording;
this small tracked fixture is a future test input and must remain.
