# N1 corrected local catalog focused validation 005

Frozen runtime/test source `22bc5f0c598aa0c78bb95926cbd418769b8dd7b6`, following actual RED004 `96f29f2aea7a3e6712ff6c199cdd642be5f887dd`. Exact source was reviewed by Huygens and Halley before execution. 449 Cargo/source/test/example inputs, 17,062,734 bytes, verified unchanged after execution.

Offline Docker Rust 1.94.1/aarch64/default stack, 2 CPUs, 4 GiB, one Cargo job, one protected private target with own-package clean and read-only shared registry. No network, home credentials, personal daemon/data or socket mounts. Container `recalld-feynman-n1-local005` exited 0 and auto-removed.

- Formatting: exit 0.
- Default library test compilation: exit 0, 47.46 seconds, 72 warnings.
- `native_catalog` filter: **38 passed, 0 failed, 0 ignored**, 1,648 filtered, 61.33 seconds. This is focused validation, not a full suite, CLI run or all-feature check.
- Executable SHA256: `ecd1db3f588751cce45170b39e824e0fe1a06fdf6928eb42ee483d615ed5059e`.
- Peak memory: 4,294,967,296 bytes; memory max events 6,489; OOM/kill/group-kill all zero. Reclaim pressure occurred, not OOM.
- Completed `2026-11-17T02:33:15Z` (November 16 local time).

The unchanged RED004 aggregate now passes all four actual cases. Before-result worker loss remains unknown with no fabricated ID/receipt and no generation change. After actual successful or committed-error result, worker loss preserves the full in-hand receipt while keeping the broader outcome unknown. After actual unknown-commit error, it preserves the already allocated operation ID without fabricating a receipt. Strict reopen, seeded ID oracle and exact unresolved work counts remain intact. Runtime does no receipt readback.

The worker records/fails its actual producer result into the existing per-request collector before the fallible tail. Join loss retains the entire typed snapshot. Checked normal projection allows zero or one validated native catalog receipt; only actual Unknown(Some(id)) supplies an unknown-operation ID. The local typed error reaches the MCP or HTTP endpoint without being flattened into a string first.

Three synthetic collector corruptions (multiple entries, conflicting replay decoration, oversized unadmitted proof) are checked through the actual adapter, actual MCP dispatcher and actual HTTP create route. Tests retain the complete typed snapshot internally, reject ordinary projection, and require bounded INTERNAL_ERROR/unknown responses with no fabricated receipt, ID or readiness. They preserve actual canonical namespace creation and unresolved work through strict reopen. These are private invariant-negative injections, not reachable fresh requests or extra canonical mutations. A defensive alternate version2 projector test also rejects these invalid snapshots explicitly; current API1 routing does not use that projector.

The prior 35 focused controls passed again, including actual emitter representation and 3.7145 receipt bytes, canonical f32 decoding, original API1 hash, oversized outer-ID prework refusal with legacy availability, candidate rollback, foreign pending outbox rejection, cancellation/worker-held publication lease, and existing catalog/allocator controls.

Historical 9ed batch rejection loss remains a separate, solved finding: exact main `200a1f10b9db52d8e5a758cab50c092ca2b07359` contains `6cbb2e53d05239c2bc68f34940666243178afbf1`; matching BatchItemRejected retains the full bookkeeping receipt and genuine unknown operations remain separate. No duplicate batch implementation or new batch runtime claim.

No global BridgeError, daemon protocol, E1 core, O1 get, ordinary startup or public2 activation changed. No full-suite, READY or publication claim. Local N1 still requires broader appropriate validation and publication review; Remote N1 requires its separately owned transport evidence/admission path. Failed004 raw/executable remains protected. Passing005 raw remains protected before PR and explicit future-input review; only compact evidence is copied here, with no raw HOME archive or cleanup.
