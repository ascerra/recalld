# Prepared native single and profile-ID batch transports

Runtime: `b6873e3` (full SHA and source/executable hashes in validation.json).
Actual parents: PR122 `abd0371`, PR123 `d7c4d5f` (including118/96),
readiness-observation `cc865c2` (direct96), and published PR121 `3fa91d4`.
This is an actual prepared consumer on owned native fixtures. Public contract2
and ordinary native startup remain closed; this is not complete A4 activation.

## Validation

| Immutable source | Checks | Result |
| --- | --- | --- |
| `4a6d04f` | Full all-feature suite | 1,213 library +2 CLI passed;1 explicit historical fixture ignored,4 doctests ignored;828.04s library |
| `4a6d04f` | Focused single and separate genuine BEFORE replay | 21 +1 passed |
| `4a6d04f` | fmt / clippy all features/all targets | exit0 / exit0 with warnings |
| `b6873e3` | Focused single | 22 passed,1 explicit ignored;20.58s |
| `b6873e3` | Genuine BEFORE123 dense receipt replay | 1 passed;19.77s |
| `b6873e3` | Actual batch wire excluding already-tested single cases | 5 passed;67.95s |
| `b6873e3` | All daemon/protocol/client regression tests | 81 passed;13.36s |
| `b6873e3` | fmt / clippy all features/all targets | exit0 / exit0 with warnings |

The only production Rust delta from the full-tested source is admission for two
new daemon aliases before dispatch. The remaining Rust delta is its test helper
and actual negative/positive fixture; see final-runtime.patch. Per coordinator
review, the full backend suite was not repeated locally for that narrow delta.
Exact-head full GitHub CI is still required before integration. The final clippy
lib-test summary reports1,042 warnings, including935 duplicates; no warning-free
claim is made.

All checks used Rust1.94.1 in recalld-ci:1.94 with Docker network none,2 CPUs,
4GiB and2 Cargo jobs. One protected offline Cargo cache and one new private target
were reused. No released path was recreated. Every tested source was an immutable
read-only export; source hashes matched before/after. All executed full-run
binaries were hashed before cleaning only the owned package for the changed
source. The final test binary SHA256 is
`4ce8b5d593ff856a31d2c448ae78327d429ea3f3f6eef379d2f3fd08021c41fe`.
Both successful containers and the failed-run containers were removed. No home,
credentials, personal daemon/data, or host socket was mounted.

## Actual transport and failure evidence

The canonical BEFORE fixture was produced by unchanged PR123 `d7c4d5f` plus the
appended test in ../before123-producer.rs.txt. Append it to that revision's
src/storage/metadata/native_records/store_producer_tests.rs and run
before_single_max_link_receipt_framing in the same isolated setup with a writable
/logs mount. Its retained artifacts feed RECALLD_SINGLE_BEFORE_ARTIFACTS through a
read-only mount for the explicitly ignored final replay test. The producer
created1,024 edges and1,025 record proofs with zero embedding calls and validated
reopen. This is genuine old native state, not the separate invalid synthetic
maximum-value serializer stress fixture.

The replayed full receipt is3,220,501 bytes; HTTP is3,221,543 bytes; MCP with
text+structured content and JSON-RPC framing is6,650,384 bytes; actual daemon
framing is3,221,546 bytes. The counting serializer's conservative bound is
6,715,808 bytes. All fit the negotiated8,388,608-byte frame. This proves this
fixture fits, not all valid histories. No receiptReference, silent receipt
truncation, changed historical hash, or invented receipt is introduced.

The fixture has an HTTP-only creation timestamp. HTTP and daemon execute their
actual prepared consumers; MCP uses its actual full-reply serializer. A separate
ordinary-fields test executes actual MCP dispatch, HTTP and framed RPC. Strict
full receipt equality, canonical bytes, provider/profile/clock counts, and reopen
are checked. The inherited environment-selected producer helper tests in broad
counts do not independently prove a historical fixture replay; the separate
ignored test was explicitly executed with the genuine fixture.

The new observed single and batch aliases require full-frame negotiation before
any dispatch. Actual unannounced,1MiB and4MiB peers are rejected with unchanged
canonical bytes/generation and zero provider/profile/clock calls. Identical
requests then commit with full receipts and ready=true after8MiB negotiation.
Existing legacy RPC names retain their original behavior; no mutation fallback
is attempted. This admission proof does not claim a reproduced fresh
commit-followed-by-overflow bug.

Known publication failures retain full receipts and actual observed readiness;
unknown applied stamps are null, pendingWork means accepted tickets only, and
raw durable categories are separate. Observation failure keeps the receipt with
READINESS_UNKNOWN. Final batch barrier failure returns committed items/full
receipts with HTTP207/MCPisError, and same-key repair performs no new mutation.
Missing private single-reply data or scope fails closed without losing an
in-hand receipt, warnings or readiness. Unknown replies do not invent receipts.

## Preserved failures and corrections

- `083d9a4` did not compile: Generation has String conversion, not Display. No
  tests ran; corrected before the prior cc4dc09 focused pass.
- `d12a9a3`:20 focused passed,1 failed. Retired replay returned its original
  receipt and ready=true but strict bytes changed in meta.db. Setup used raw
  purge/create core methods without owner completion. The corrected fixture
  asserts stale publication, runs the real completion owner, asserts unchanged
  canonical generation and all4 current views, then keeps strict replay bytes,
  provider/clock and retired-ID assertions. Runtime was not relaxed.
- `cb81448`:21 focused passed; the separate historical RPC replay failed because
  its helper omitted ping and was correctly limited to1MiB. HTTP/MCP equality
  passed before that assertion. Test negotiation was corrected; production
  limits were not enlarged. The subsequent independent server admission review
  led to the two-alias pre-work guard and actual negative/positive regression.

Original failed sources/logs and hashes remain identified in validation.json.
Raw runs are disposable under the user's retention override after the resulting
PR and recorded results; no new HOME raw archive is required. Code, reports,
tracked evidence, worktrees/.git and active future fixtures remain protected.

## Remaining scope / required companions

- Curie's fresh native document-v1 recipe correction and actual provider-input
  tests are required. Earlier experimental generated vectors are not certified
  for recipe/retrieval quality; old hashes/receipts must not be rewritten.
- Native API1 linked-insertion/batch context and public response projection,
  including the approved explicit profile-ID requirement for supplied native
  REST vectors, remain dependent work. Legacy-disk None compatibility remains;
  an explicit selector must not be ignored.
- Typed core caller-vector validation is required for precise dimension422
  mapping without string parsing. Provider failures remain unavailable.
- Legacy-disk API2 lifecycle/compound consumers, ordinary startup writer closure,
  actual A2 query/similar selection, final schemas/discovery and whole public
  MCP/REST/RPC contract tests remain activation prerequisites.
