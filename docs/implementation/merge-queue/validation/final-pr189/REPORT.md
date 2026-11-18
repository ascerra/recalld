# PR189 current-main integration — 2026-11-18

Frozen owner head35c26a5319eb4f87d004d9d68695bfdd53519a51 was reconstructed in
the queue ledger before integration. Prerequisites103 and186 are merged.
Actual main0800e432b95a4c21530f8aaec437536da5570f99 has all14 CI checks green
(run44362821613). Normal merge2576ef1672f4a96dd0cead2775534b2743583ad4 preserves
both histories. One server dispatch conflict retained main's request context
for tools/list and N1's request ID for tools/call. Source delta against main
remains exactly the original20 files in changed-source-files.txt.

Initial composed compile failed E0061 at two old NamespaceRegistryAdapter::new
calls in catalog_api1_tests.rs. Follow-up e7514f165611f4238ed49c17e1aa84c369b3c50c
passes each fixture's existing provider.clone() to the current constructor.
No runtime behavior or assertions were weakened. Original compile failure is
retained separately. This final source is the tested source; report tail is docs only.

Actual focused test command:
cargo test --offline --locked --all-features --lib -- native_catalog api1_catalog
mcp::server::tests:: performance::tests::writes::batch:: format_open_tests --test-threads=2

PASS71,0failed,0ignored,85.50s; test compile1m56s. Includes unchanged
performance::tests::writes::batch::legacy_batch_actual_partial_stdio_requires_parent_and_child_facts,
catalog candidate budget rollback, known/unknown/invalid worker evidence,
actual HTTP/MCP responses, cancellation, legacy routing, format detection,
strict preflight, malformed/missing metadata, namespace allocation and replay.
Formatting PASS (installed recalld-ci:1.94 formatter); source diff --check PASS.
The broad merge diff contains preexisting evidence-log whitespace from main;
those historical raw files were not rewritten. No new full local suite claim.
Fresh all14 current-head CI and current-main green remain merge gates.

Runtime: offline linux/amd64 Rust1.99.1 pinned image
rust@sha256:e6f2cff35d0f309acf8433b4f05878ed77092602c5065cc81801177da0bb3a9c,
default stack,no RUSTFLAGS,incremental0,jobs1,2CPU,4GiB RAM/8GiB RAM+swap.
Read-only composed source and explicitly transferred private cache only;
no personal home/data/credentials/socket mounts. Held diagnostic003 untouched.
Containers auto-removed. After final tests, queue releases its x86-target and
private Cargo registry exclusively to Feynman for separately authorized release
migration work; further queue builds require a fresh ownership handoff.

## Release boundary

This PR preserves local legacy catalog routing and requires response admission
for an already-owned native catalog. It does not activate ordinary native
startup or migrate an installed user's database. Format tests use isolated
current-code fixtures, not a released-v1.0.0-produced database roundtrip.
The new release requirement (durable backup, migrate copy, validate/cutover,
rollback/fail startup, no legacy serving) is separate authorized work. Native
semantic compatibility for UnknownLegacy provenance must not be inferred or
bypassed. No release publication or full migration compatibility claim here.
