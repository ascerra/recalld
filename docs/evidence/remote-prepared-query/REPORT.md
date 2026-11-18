# Prepared Remote query consumer

Runtime: `09399ad1811396e17ad85e1b735c6d1334e75fb5`.
Actual parent184: `5d18075eea193849056ae88cb07f27843d1c7019`.
Owner: Huygens. No catalog/list/ordinary-startup convenience parent.

The existing prepared query, similarity and exact-pair bridge methods now reach
their actual owner through the daemon. Negotiation, response admission and
receipt classification use the same connection, with no post-send retry or
legacy fallback. Public contract2 admission remains closed; v1 shapes/defaults,
comparison kernels, stored bytes and global serde numeric behavior are unchanged.

## Results

- 31 distinct focused tests passed on09399:24 prepared consumers/peer controls,
  1 maximal-valid lifecycle receipt bound,5 exact-pair tests,1 whole-frame test.
- Formal JSON Schema validation passed56 focused fixtures, then63 full-run
  input/output cases plus mutation negatives. Runner and fixtures were read-only
  and verified unchanged. These counts are not additive unique-case counts.
- Frozen full/all-feature:1,706 library tests and10 CLI integration tests passed;
  3 existing library ignores and4 ignored doctests. Formatter andClippy exited0
  with warnings. Library execution took2,068.21 seconds under the stated limits.
- Native false controls use actual migration, explicit policy adoption and
  re-embedding. Query, similarity, exact pages and a true native publication-fault
  control passed, including the full retained receipt after ordinary recovery.
  This is owned-fixture proof, not ordinary native startup or public2 activation.
- Maximum-count authored serializer fixture:16,256 pairs; private frame2,199,718
  bytes against F2,795,934. Real query1MiB positive: daemon1,052,974,
  MCP-inner2,106,330, REST1,052,878 bytes. Real2MiB false/true requests refused
  before exposure and preserved canonical state. These are bounded correctness
  checks, not an A5 latency/performance acceptance result.

## Failures retained and corrected

Run001 proved the actual legacy owned query200 versus Remote503 boundary. Run003
proved six decoder/namespace defects with unchanged genuine producer controls.
Run004 was compile-only E0308; run005 exposed the native mixed-case display
assertion. Runs006 and007 were distinct fixture failures: direct `to_value`
expected a different f32-derived decimal representation than the actual emitter,
then unsupported `fts_only` stopped the next case before execution. Feynman
reviewed the exact test-only emitter correction. Strict Value equality and
canonical f32-bit checks remain; no tolerance or serializer change fixed006.
The supported FTS case is hybrid with an explicit vector/profile, not pure FTS.

Runs008/009 subsequently proved a genuine f64 transport defect. For an actual
cosine with bits4606822130811788905, equal-threshold transport changed the server
threshold to8906 and omitted the pair. Below-threshold transport also changed
bits and invalidated continuation. The response first drifted in `compact_value`.

The separately reviewed production correction uses private fixed16 lowercase
hexadecimal `thresholdBits`/`similarityBits`, a new `exact_pairs_bits_v1` alias
and an explicit `exactPairsBitsV1` same-connection capability. Old peers/obsolete
aliases fail closed. Strict private DTO validation precedes restoring f64 values;
there is no decimal reparse after restoration. The original kernel and cursor
format are unchanged. Run010 proves below/equal/above local/Remote page and bit
identity, all three actual returned-cursor continuations, malformed/nonfinite/
conflicting fields, old-peer/refused negotiation, and unchanged canonical bytes.

## Evidence boundaries

`receivedEvidence` is explicitly unverified evidence from a malformed Remote
wrapper, not a receipt for the current query. Runtime requires complete Rust
WriteReceipt decoding and exact canonical roundtrip. Its separate JSON Schema
requires the six headers and an opaque outcome with a string kind; it does not
claim complete outcome-schema validation or authenticity. Normal successful
observation schemas remain strict. A sent observing request stays unknown when
its reply is unattributable; no current receipt/identity/readiness is invented.

The actual BatchReserved peer fixture is persisted and reopened before use.
The large quote-heavy malformed receipt is separately authored codec-valid peer
data, not a canonical producer maximum. Exact F/F+1 tests distinguish full
retention from pre-decode refusal. The complete frame at F measured8,388,010 bytes
against8,388,608. The bound covers inner ToolCallResult and daemon framing;
arbitrary outer MCP string IDs need separate admission before public activation.

Expected-frame artifacts reproduce the actual emitter with a synthetic maximum
request ID. Received-projection artifacts are final projected bytes, not raw
network taps. Run009's server threshold comes from its actual returned cursor;
its raw-client diagnostic exchange is distinct from the actual Remote call.

## Reproduction and provenance

Offline image `recalld-ci:1.94`, `RUSTUP_TOOLCHAIN=1.94.1`,2CPUs/4GiB/jobs1;
no network, home, credentials or daemon socket mounts. Source is read-only and
the single owned Cargo/target is reused. Source switches clean only this package.
Run010 and011 use the same unchanged source root and target, without a switch.

Focused filters: `prepared_`,
`strict_query_valid_lifecycle_receipt_groups_fit_full_error_projection_reservation`,
`exact_pair`, and
`exact_scan_response_budget_covers_real_mcp_text_structured_daemon_and_rest_serializers`,
each under `cargo test --locked --offline --all-features --lib FILTER -- --nocapture`.
Full command: `cargo test --locked --offline --all-features`; formatter:
`cargo fmt --all --check`; lint:
`cargo clippy --locked --offline --all-features --all-targets`.
Formal runner: `python3 tests/query_schema_conformance.py FIXTURES` with existing
offline test-only jsonschema4.25.1.

All452 committed build inputs matched the immutable source after execution.
Fourteen produced/used test and CLI executables were hashed, including every
`CARGO_BIN_EXE` reference in the integration tests. The focused/full library hash
is identical: `8df6e430a398c67be9b85dbc7182143b5a2e3b1fc195b1a5cd80fb09c4b97ff9`.
Image ID: `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
Full-run cgroup peak4,294,967,296 bytes, max events2,296, OOM/kill0: reclaim
pressure, not an OOM. No retry with changed resources or recreated target.

[Development results](development-runs.json) retain exact sources, log hashes,
executable hashes and failure attribution. Run001's9,198-file archive includes
tracked historical evidence; later452-file manifests are actual build inputs.
[Implementation notes](../../implementation/remote-prepared-query.md) and the
[reviewed scalar plan](../../implementation/remote-exact-scalar-plan.md) give the
scope/owner handoffs and exact009 bit evidence.

## Remaining gates and cleanup

Exact-head CI and final queue composition remain pending. R2 metadata
pagination/session consumers, outer MCP request-ID budgeting, ordinary native
startup/writer closure and coordinated public2 activation are separate work.
Final integration must retain actual177 same-view source/donor certification;
this Remote consumer does not replace those guards or remove that integration gate.
No full A4/spec-completion claim. No personal recalld access or deployment.

Completed containers001 through011 and schema containers were removed. All R1 raw roots remain centrally
protected until publication/results and explicit release; no raw-root deletion
or quantified space-recovery claim. The sole active Cargo/target is retained.
No new HOME raw archive or per-run cache clone was created.
