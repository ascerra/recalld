# Prepared Remote canonical pages

Frozen validation source: `7eec6f48e512f91f2bcd08357eba85089ffffb53`.
Owner: Huygens. The coordinator approved this isolated consumer under the
user's standing implementation authorization. Halley owns first push/create
after the final frozen handoff; this report does not itself publish the branch.

Actual parents are canonical pages #112
`bcbd4fe63409945a0a297d52c165ac3060fe3de7` and prepared Remote queries R1
`52c9bdf982270a576576c0a67164dbd64e322482`, joined without conflicts at
`3991e45b47df1a02fec559499a2c5246f4222ac1`. There is no #65, N1, E1 or later
main convenience parent. These exact parents remain ancestors of the tested
source; no queue worktree or published frozen branch was edited.

## Behavior

Remote canonical memory, namespace and tag pages now reach their existing
same-transaction owners through private daemon methods. Capability selection,
negotiated frame admission, strict decoding and response correlation use the
same connection, with no fallback or post-send retry. Metadata capability is
independent of prepared-query support, active provider identity and vectors.
Paging performs no observation and invents no receipt or readiness state.

The backend retains canonical GenerationRef, scope, hash, filter-before-page,
anchor, exact-total and ordering authority. Remote validation checks consistency,
not authenticity. It preserves exact Unicode and canonical-empty/provenance
labels, validates carried profile compatibility, and preserves an explicit
carried Unknown configuration over an outer caller's configuration. Count
validation uses checked `returned + hasPrior + hasMore <= total`; a prior cursor
does not reveal its ordinal. Output overflow is a whole error, not truncation.

Public/v1 payloads and defaults, cursor formats and backend selection semantics
remain unchanged. Public contract2 admission is still closed. The shared
catalog label-anchor helper keeps its original hashing bytes/domain and moves
only to let the strict consumer use the same implementation.

## Validation

Run003 (`fd2e977a`) passed20 distinct focused tests. Run004 (`6ae99ee0`) passed23
focused tests and formatting, then its separate formal schema validation failed.
Those results are not relabeled as final-source execution.

Run005 (`7eec6f48`) passed four affected fixture tests and formatting. Formal
JSON Schema validation passed12 actual emitted envelopes, covering success and
error for each of list, namespaces and tags, plus shape/counter mutation
negatives. All468 source inputs and13 formal inputs were unchanged. The full
run uses that same immutable source root and target without a source switch.

The frozen full/all-feature run passed1,733 library tests and10 CLI tests,
with3 existing library ignores and4 ignored doctests. The four binary unit-test
targets contain0 tests. Library execution took2,067.09seconds. Formatter and
Clippy exited0 with warnings. Formal validation then passed22 full-run actual
page envelopes plus strict mutation negatives. All23 formal inputs (22 cases
and the runner) were unchanged. The12 focused and22 full-run cases are separate
collections, not an additive unique-case count.

The focused controls cover203-row continuation with changed page limit,
filter/generation rejection, ordered namespace/tag pages, uneven bucket
exhaustion, actual native Unicode/authoritative-empty/provenance and reopen,
configured profile context, and malformed peer/count/scope/profile/capability
frames. Actual Unix-socket Remote results are compared with prepared MCP
projections and owned HTTP results; public contract2 routing is not activated.
Provider/clock panic controls ensure the metadata path makes no such calls.

The width8193 test deliberately authors an already-open legacy metadata row
after proving actual fresh creation still rejects that width. It is a reader
inventory fixture, not proof that a historical writer accepted that row,
ordinary reopen permits it, or native publication is valid at that width.

The escaped-field legacy fixture rejects the whole40-row page, then succeeds
with limit10. Actual serialized sizes: REST201557bytes; modern MCP456169 and
daemon456062; legacy MCP254591 and daemon254484. Measurements include wrappers
with maximum numeric IDs. They are fixture measurements, not an all-history
maximum, arbitrary outer MCP string-ID theorem, RSS limit or A5 latency result.

## Preserved failures

- Run001 executed the actual missing Remote consumer:0PASS/2FAIL while owned
  list/namespace/tag pages succeeded. Canonical record/control assertions and
  socket cleanup completed before the intentional failure assertions.
- Run002 executed all three malformed-count cases on genuine owner replies:
  continued list and namespace totals3→2, and first truncated tag total3→1.
  All were accepted before correction. Result4PASS/1FAIL including the separate
  actual native transport control; the failed executable and log are retained.
- Run004 formal validation rejected actual `not_found` because the newly
  declared schema incorrectly used `notFound`. The actual production enum is
  snake_case. The correction derives schema values from the typed enum; it
  changes no production serializer, parser, numeric tolerance or payload.

[BEFORE](BEFORE.md), [count RED](COUNT-RED.md) and [focused history](FOCUSED.md)
record exact sources, executable hashes and the distinction between source
review and executed evidence. Feynman reviewed backend/cursor/decoder contracts,
the count correction, schema correction and stated limitations. Halley reviewed
the actual capability/context/dispatch seams. Neither source review is presented
as independent execution of the full run.

## Reproduction, scope and cleanup

Offline `recalld-ci:1.94`, Rust1.94.1,2CPUs/4GiB/jobs1, normal test parallelism.
No network, home, credentials, personal store or daemon socket mounts. One owned
Cargo cache and target are reused; source-root changes clean only the recalld
package before compiling. Source, formal fixtures and validator packages are
read-only, with separate writable evidence output. No global serde change,
cache clone, deployment, benchmark retrieval run or personal recalld access.

Full command: `cargo test --locked --offline --all-features`. Formatter:
`cargo fmt --all --check`. Lint:
`cargo clippy --locked --offline --all-features --all-targets`. Formal runner:
`python3 tests/page_schema_conformance.py FIXTURES` using the existing pinned
offline test-only validator. The full driver's SHA256 is
`69a2acd06aa88a0c2f7928de6d666905db7582b1aa234752cd9fe89717715016`.

All468 committed build inputs matched after execution. Fourteen produced/used
test and CLI executables were hashed, including all `CARGO_BIN_EXE` references.
The focused and full library executable hash is identical:
`2c81ca5846e98f0af8337286e0396685a6721590e2751b8a6a35b6a4d693f917`.
Image ID: `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
Full container UTC22:19:18–22:57:11 on2026-11-16; cgroup peak4,294,967,296bytes,
max events1,804, OOM/kill0 and container OOMKilled=false: reclaim pressure,
not OOM. No retry, resource escalation or second full run was required.

[Development runs](development-runs.json) preserve source-specific earlier
results/log hashes. [Full validation](full-validation.json) records results,
resource limits, all executable hashes and final formal validation; the
[source manifest](source-manifest.json) records every tested build input.
All owned Rust/schema containers and the hash collector have completed and
retired. The final handoff adds documentation/evidence only to tested7eec.

Exact-head CI, queue composition with actual177 source/donor certification,
ordinary native startup/writer closure, arbitrary outer MCP request-ID admission,
public2 activation and A5 page-latency acceptance remain separate gates. This
slice does not claim full A4 completion or a generic coherent ReadView.

Raw roots001/002/004 remain protected failures;003/005 remain protected until
publication/results and central release. The sole active Cargo cache is
`/tmp/recalld-implementation/compaction-review-7281918/cargo`; target is
`/tmp/recalld-huygens-active-validation/target`. Central cleanup owns deletion;
no raw roots were deleted here and no recovered-space claim is made. Worktrees,
Git metadata, tracked reports, future fixtures and personal data remain protected.
See [implementation notes](../../implementation/remote-canonical-pages.md).
