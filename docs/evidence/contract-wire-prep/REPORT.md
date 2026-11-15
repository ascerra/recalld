# Wire preparation validation

Frozen runtime `7dea9d201fe7528b3dc97abd1d8b5944e8feab96`, branch impl/contract-wire-prep. Minimal source
parents: PR69 eefc987 plus PR13 1b23532 (merge b4742ef), then UTF8 binary-hash
evidence 10986fe. PR72 batch and PR73 tracker are not imported. No push/PR;
this content folds into the first actual MCP/REST contract consumer.

Eight focused tests and the original omitted-arguments regression passed.
Full normal-parallel `cargo test --offline --all-features`: **833 library + 2
CLI integration tests passed**, zero failures; library duration 214.88s.
Four doctests were ignored. `cargo fmt --all --check` passed.
`cargo clippy --offline --all-features --all-targets` exited 0 with 431 library
and 455 library-test warnings (431 duplicates), including a collapsible-if
warning in the new helper. No warning-free claim.

## What failed before the final run

- Initial focused run: 6 pass/2 fail. The real HTTP insertion fixture required
  a multithread Tokio runtime; list_namespaces is a legacy array, not an object
  structuredContent result. Tests were corrected; namespace_stats supplies the
  actual object-result regression. Existing production payloads were unchanged.
- Initial full run: 832 pass/1 fail (214.79s). The new dispatcher guard changed
  the existing explicit-null object-error text. The actual source fix reuses
  args::require_object first. The old regression was not weakened or rewritten.
- The added exact error-payload comparison then found that modern dispatch
  adds protocol resultType/_meta outside the tool result (7 pass/1 fail).
  The test now compares complete content and isError, checks no structured
  content, and separately asserts resultType=complete.

Final tests reject unsupported/malformed MCP selectors on all tools and REST
headers (including duplicates) before malformed body extraction, provider calls
or mutation. Actual fixture file bytes and control state remain unchanged.
Real legacy insertion/read routing and MCP dispatch preserve v1 shapes, arrays,
unknown-field tolerance, omitted no-argument calls, and legacy-era text-only
results; modern object text and structuredContent match. Pure fixtures cover
strict decimals, key/header agreement, original input preservation, JSON Pointer
escaping, batch partial counts and envelope/receipt/readiness serialization.
Typed envelope values are authored serializer fixtures, not native publication,
readiness, complete ReadView, or all-writer proof. No external provider is called.

## Isolation and executable provenance

See container.json for actual image ID, mounts, 2CPU/4GiB/network-none and Cargo
settings. Image recalld-ci:1.94 uses RUSTUP_TOOLCHAIN=1.94.1, CARGO_BUILD_JOBS=2.
Only assigned source, this agent's private cargo/target, and output mounts were
used; no home, credentials, personal store or socket mounts. No source changes
occurred while Cargo was compiling/testing it; source corrections were sequential.

Before the source root switched from canonical-label-consumers to this worktree,
all six prior executed UTF8 binaries were hashed in 10986fe. Then this agent ran
`cargo clean --offline -p recalld` in its OWN target (4476 files/12.0GiB; dependencies
preserved). All prep revisions stayed on this same source root; there was no
BEFORE/AFTER/switch-back comparison. The initial, intermediate and final executed
binary hashes are preserved separately. Fresh PR69 CI proof is independent;
this audit does not invalidate unaffected validation.

Exact commands after the one package clean were:
```
cargo fmt --all
cargo test --offline --all-features contract_
cargo test --offline --all-features a4_optional_tool_arguments_preserve_no_arg_calls
cargo test --offline --all-features
cargo fmt --all --check
cargo clippy --offline --all-features --all-targets
```
All were run inside the isolated container described above. The final all-feature
suite also runs the PR13 modern-structured/legacy-text regressions and prior
native/legacy format guards. No separate public runner/episode suite was claimed.

## Remaining actual consumer

Contract 2 unconditionally fails closed and is not advertised. Helpers do not
activate public keys/CAS, receipts, native serving, ReadView, pagination or session
capabilities. The intended next consumer is MCP/REST exact pair pagination,
strict operation schemas and discovery/tool guidance, joined with Feynman's real
canonical/vector capture and post-comparison generation/policy/lineage guard.
Actual native transport acceptance awaits Pascal's owner/serving join. No new
offline CLI and no unused-helper PR. The implementation note records the 201+,
cross-page, dense>10, tied/disconnected, restart and mutation-during-page oracles.
Full A4 is not complete.
