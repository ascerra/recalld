# Live delete output schema: frozen affected validation

2026-11-17, Huygens. Measured schema/test source:
`77126d1b5c85c9f21c986c509bfc4d66ecdd0652`. The final tail is documentation,
compact evidence and notebook verification only; no test repeat or full-suite
claim accompanies that tail.

## Actual change and parent

Add modern delete_namespace outputSchema describing existing typed success and
three structured purge failures. Modern object schemas are now12of14. Full
start/step/final receipt evidence is preserved; unknown current operations do not
become committed because prior evidence exists. Missing physical metrics and
readiness are not invented. Legacy plain-text errors remain unchanged.

The sole actual parent is PR198
`6a615b82dbdf18448e3bea8dc7b715305ad1266e`: its typed purge receipt definitions
are directly reused. Existing main purge producer/parser is already in that
ancestry. No189/N1, unfinished native Remote catalog, new protocol, serializer,
producer, receipt format, registry or public2 activation change is included.
Halley released this isolated schema scope; the coordinator approved it under
the user's standing authorization. Parent and queue branches remain untouched.

Source map: new src/mcp/tool_catalog_schema.rs (61lines), registration and existing
tools/list expectation; optional captures in existing actual purge/legacy tests;
tests/live_delete_schema_conformance.py; implementation/MCP docs. Production
mutation/query implementations were not modified. Create remains explicitly
without an output schema: any JSON object currently carried by PartiallyApplied
is emitted. A catch-all alternative would make a strict-looking schema ineffective.
A separate owned compatibility decision must address that actual bridge boundary;
merely adding native Remote creation cannot constrain the legacy lane.

## Source-review correction before freezing

Feynman found unknown operationId uses Rust's UUID parser but returns the original
string. Simple32hex, URN and braced spellings can therefore be emitted. This field
is documented nullable string rather than falsely requiring canonical dashed
format. Existing typed receipt UUID constraints remain unchanged. The existing
actual parser regression now checks those three spellings of the same genuine
unknown operation ID while preserving its full receipt and unknown state.

This was a source finding corrected before candidate freeze, not an executed RED
or a production parser bug. Feynman source-reviewed the three alternatives,
receipt/scope dependencies and stage restrictions; this was not an independent
execution claim. Final schema file SHA256:
`dfdae84b63b0ef1877a9285d52bb3b46574ce3dd92bcb0820a5185157fc9f699`.

## Results and limits

| Check | Exact result |
| --- | --- |
| Offline all-feature lib compilation | PASS,65warnings |
| Existing affected Rust regressions | 9passed,0failed,0ignored |
| Formal Draft2020-12 output checks | 17captured cases plus meaningful malformed-shape negatives PASS |
| cargo fmt --all --check | exit0 |
| cargo clippy --offline --all-features --all-targets | exit0 WITHwarnings |
| Source verification | all518file hashes match after run |
| Executable verification | identical before/after and afterClippy |

Test executable `/target/debug/deps/recalld-8b31964f64b2cfc4`,653809064bytes:
`198aac62c0cbb7bf98d74693d5097d1321e544172e810feae7088f7028e71fa2`.
The filename can match another build; the content hash is this source's distinct
binary. Before rebuilding, cargo clean -p recalld --offline cleaned only the owned
package target. No stale executable or source-switch proof is reused.

Actual cases cover native purge success/physical metrics, forced competing
completion through modern/legacy MCP and capable Remote/legacy daemon frames,
start and child full receipt failures, unknown worker/child operation IDs and
reopen controls, actual parser malformed numeric/nested/identity rejection,
legacy forced deletion, plain legacy partial/ordinary errors, and tools/list
modern/legacy behavior. Existing assertions remain. The three UUID-spelling cases
are authored representations of the same real unknown operation, not three new
commits. Shared typed-emitter success captures are labeled as such rather than
claimed as additional public invocations.17outputs include text/structured and
shared projection captures, not17independent histories.

No new failure occurred in this candidate. Existing parent failures remain in
their original reports and are not erased by these affected passes. No full suite,
new exhaustive matrix, high-peer capacity rerun, retrieval or performance run was
needed for this schema-only change. Exact-head CI and final composition remain
Halley's integration gates.

## Reproduction, resources and cleanup

Immutable read-only source archive of exact77126d1b with required fixture files;
source-sha256.json and source-verification.json pin it. Image recalld-ci:1.94:
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
Rust1.94.1, CARGO_BUILD_JOBS=2, Docker --network none --cpus 2 --memory 4g. No
home/credentials/socket/personal data mounts. Same one private Cargo/target reused.

Compile cargo test --offline --all-features --lib --no-run, then focused.py runs
the exact executable for nine recorded filters, --test-threads=2. Set
RECALLD_LIVE_RECORD_SCHEMA_FIXTURES=/evidence/fixtures. Formal checker uses the
existing offline Python3.11/jsonschema cache:
python3 tests/live_delete_schema_conformance.py /evidence/fixtures.

Both build and checks peaked at4294967296bytes. Build memory.events max38134;
checks max5502; both oom0/oom_kill0/oom_group_kill0. Reclaim pressure is not an OOM
failure. No performance conclusion. Raw logs/hashes remain source-attributed.

Active run: /tmp/recalld-huygens-active-validation/live-delete-001-77126d1b;
source-only formatter: /tmp/recalld-huygens-active-validation/live-delete-format-001.
After publication and recorded results, release both centrally. No raw HOME archive
is required; NOTEBOOK-COPY.json records the compact dated append. Preserve all
protected previous failures, worktrees/.git/reports and one private Cargo/target.
Actual recovered bytes are not yet known; no deletion/recovery claim is made.
