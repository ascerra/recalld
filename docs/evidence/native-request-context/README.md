# Original request identity: native catalog, policy and compound purge

This is an actual PR121 consumer child. Shared `MutationContext::validate` already
accepts1|2 and is unchanged. Standalone native_records, native_lifecycle and
native_maintenance are unchanged and do not require this parent. Only compound
operations need the new retained request context; batch consumes this codec in
its later actual consumer. Ordinary startup and public routing remain gated.

The native owner selects native purge codec/domain independently of the original
public API. Catalog/policy keep their native guards and use incoming contract1|2
in the original request hash. API1 purge retains a closed optional context plus
force and original CAS. API2 omits the new field, preserving historical bytes.
Unknown fields/versions, Some2 and explicit null fail closed. Missing context on
the known old native codec must match exactly one of four historical hashes:
force=false|true crossed with CAS absent|original previous generation.

Fresh replay_or_check compares supplied CAS with transaction control generation
before mutation. record_receipt advances that same transaction once. The start
proof independently checks previous_generation+1 == committed_generation. This
bounds historical candidates without using a fork's current generation, guessing
general provenance, or rewriting old receipts. One canonical start helper serves
strict offline/startup validation, resume, replay and current receipt checks.

## Genuine BEFORE and negative candidate

BEFORE source1b4eb26f6c05ae12186ac5826a4f5d6f642f9118 adds only the attached test
exporter to actual runtime parent3fa91d4. The genuine old binary authored four
force/CAS combinations, each at seq0 with no child, then completed, replayed,
exported/imported and fork-restored them. All four retained exact original start
receipt bytes. Export test PASS28.02s; binary SHA256
8b9ed78bd73af639e2a02ded1f4e3c806db2703ba95e8a8875d683b19e3a2a50.
The 180-file fixture manifest and compact original receipt/case files are retained.
Database artifacts remain outside git in the sealed validation directory.

The deliberately vulnerable *proposed new API1 candidate* f79614e removes only
the four-hash check from cb6a452. It is not an existing production exploit:
published API1/native support was unsupported. Actual process exit after purge
start leaves seq0 and start==last. The probe removes request from all three
canonical/embedded copies without changing requestHash. Candidate preflight
incorrectly accepts, causing unwrap_err to fail: 0PASS/1FAIL1.37s, binary
SHA256d77f011accb6d8b29dc9f943044a65e719b93c9895b95acef69a046f73f4c492.
Corrected cb6a452 rejects this exact state and preserves source bytes; intact
crash state resumes. The attached patch identifies the sole negative change.

Eight focused tests PASS12.85s on cb6a452 (runtime d933e5f), binary SHA256
a6a1f06bf4d6576875277ca39764e694856492fe8abdc47eecd1f9b609d2972b.
This includes one process helper, all four genuine BEFORE combinations across
started/completed/forked artifacts, ten all-copy malformed proof variants,
genuine older-binary rejection of the new field, catalog/policy replay and
cross-contract conflicts, keyed no-change replay after another policy update,
unkeyed API1 catalog/purge, export/import/fork and current four-view readiness.
External genuine BEFORE checks are enabled by the explicit environment in the
recorded run; ordinary CI skips those external artifacts, not the self-contained
proof tests. Counts are test executions, not independent repetitions.

## Earlier failures and provenance

- ffe1edc: compile failure from test DiskRecord PartialEq use; no executable.
- c48b677: compile failure from unwrap on Vec-returning to_bytes; no executable.
- 54d22a5: six focused passed; genuine BEFORE fixture failed because the existing
  vector inspector requests a writable file handle. Sealed originals were
  read-only; a6cbb6d uses writable private copies and checks their bytes.
- a6cbb6d: seven focused passed11.37s.
- One formatter-only retry observed a transient incomplete Docker bind-mount
  view immediately after the host wrote a test file. Host file was complete;
  no runtime test ran on that formatter failure. Final runs hash-check every
  tracked file inside the container before and after compilation/testing.

Corrected, genuine BEFORE and negative candidate use separate source roots and
separate private targets. Negative target was package-cleaned before compilation.
No source-root switch reused an executable. All runs use recalld-ci:1.94,
RUSTUP_TOOLCHAIN1.94.1, Docker network none, two CPUs/4GiB, build/test jobs2,
debug information disabled, private targets and a read-only registry. No personal
data, credentials, daemon or socket mounts. Source and BEFORE artifacts are
read-only in final runs. Exact scripts, per-run source/tree/log/executable hashes
and compressed unmodified logs are attached. No Rust repeat for evidence-only
commits.

Coordinating-agent bounded source review of d933e5f found no blocker in the
shared codec, four-candidate proof, native owner/context separation, canonical
anchors and current/resume paths. This is source review, not additional runtime
validation. Subsequent review identified that the new begin-replay validation
could lose an already-in-hand receipt; its narrow correction and actual proof
are recorded in the final report, without reconstructing or rereading evidence.

## Actual followup failures and corrected focus

Frozen cb6a452 full suite:1100 passed/1 failed516.61s. CLI and Clippy were not
reached because cargo test failed. Test-only574850d exposed the exact existing
unknown-child regression error: RecoveryRequired for pending purge. The new
coordinator had used an ordinary control read, which correctly blocks active
purge; that was the wrong admission seam for recovery.

Runtime a4acd5f uses a private immutable constructor-mode check while holding the
worker's existing publication/graph/storage ownership. The same actual purge
write transaction still checks current control schema under its mutation guard;
ordinary reads, unavailable handles and canonical-pending guards are unchanged.
Unknown-child regression then passed1.57s, preserving the reserved operation ID
and unknown classification through recovery. The same executable exposed the
new known-receipt regression: bare InvalidControl after successful replay lookup
of a damaged-scope progress row (one FAIL1.10s). Source/binary/log manifests retain
both outcomes separately. a4 test executable SHA256
558c4dc34b71d6f438d9cf18617d9aabcefe1cb349f012b2bbf0b40d3ba17b68.

9e70229 wraps only progress-load/start-validation errors after replay returns the
exact receipt, carrying that already-in-hand receipt unchanged except the
original replay flag. Prelookup failure does not invent a receipt. Actual final
focused source a9053ef1287b0b2a1a266c7c72268ad61f84291b (formatted9e plus docs)
passed unknown-child1.90s and the full known-receipt matrix8.83s, then all ten
context tests24.37s. The matrix covers API1/API2 damaged start scope, missing
progress, and missing canonical receipt before lookup; it checks exact receipt,
retained raw bytes, unchanged metadata and G. A separate both-contract test
rejects a legacy owner at the native coordinator. The known-receipt check is
repeated in the ten-test filter; it is not an independent additional scenario.
Final focus executable SHA256
06d68a6608425bfaa628df7ce55724a7e52e52d6d5e7c685d5996713d726df6a.

All own heavy work drained for the explicit measurement window. During HOLD,
only source/docs changed; no formatter/build/test/container/copy ran. Validation
resumed only after explicit END22:14UTC. The stored source pointer identified
a4 before the queued executable hash was read, so later source-only commits are
not falsely attributed to that earlier binary. Final full results follow below.

## Final validation and scope

Final validated source a9053ef1287b0b2a1a266c7c72268ad61f84291b, tree
fa4fed7d18d206304edcc15984cad0713289a09a. Full default-feature runtime suite:
1103 library PASS715.68s, two CLI PASS2.59s/5.13s, four existing doctests ignored.
Formatting and default all-target Clippy exit0 (869 library/929 test warnings,
840 duplicates). The separate all-feature test build (`--no-run`) and all-target,
all-feature Clippy also exit0 (890 library/950 test warnings,862 duplicates;
main binary one warning, duplicated for its test target). The optional bench
feature adds module/command code but no tests and no conditional core branches
on this source; its targets were compiled/linted without repeating the full
runtime suite. This is not a claim of executing an all-feature full suite locally.

Full raw log SHA256
e011a4a1056625824f640031c760f8df7dac404b73167b59c889f71834c3bc38.
Feature compile/lint raw log SHA256
e26beb790fdd42f2dfc350b07d48a205de45b5cb5652303c32d57db7793a5ce7.
Executable hashes distinguish the actually executed default-feature binaries
from the separate all-feature compiled binaries. The final manifest retains both.
Source hashes passed inside the container before and after each final run.

Final combined source review confirms immutable native owner mode, worker-held
publication/graph/storage lifetime, current-schema comparison under the purge
mutation guard, independent original API validation, and exact in-hand receipt
wrapping only after successful lookup. No ordinary read guard was weakened.

No native_records/native_lifecycle/native_maintenance/control validator edit,
ordinary native serving activation, public API2 activation, legacy-disk API2
writer widening, broad ancestry merge, or power-loss claim. Compound batch and
public adapters consume the actual codec later; standalone writers do not need
this parent for coordination. The next assigned actual124 child must route the
three new catalog/policy receipt calls through the owned allocator and preserve
owned purge/batch reservations. All ownership and final activation boundaries
remain in the implementation plans, with API1 startup preceding public API2.
