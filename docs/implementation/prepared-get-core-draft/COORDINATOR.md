# Actual point coordinator and direct consumers — first validation draft

Base remains19bd383a75cdb15283b4c15a865dfcb2a4b6e0a1; helper/worker source7939d6d5bf3879ca2a9bc115fb5863f53b1c6949 received bounded A1/A3 source ACKs. No main, frozen PR, RPC/client/server/capability or personal store was changed.

Actual QueryEngine::get_contract now drives the borrowed canonical capture, operation-local projection, get-specific reply admission, existing captured-intent observer, publication and readiness. Actual MCP/REST SearchPipeline adapters and prepared inner handlers call it. Public application2 admission remains closed and legacy handler shapes stay unchanged. These are source changes, not a helper-only shipping PR or completed O1.

Plain and marker capture does not acquire graph. Explicit Full/Summary capture first discovers the need, releases engine.read, acquires graph under the actual derived-publication lease, and recaptures at the same full GenerationRef. Only this authoritative capture obtains donors, native intent and one clock sample. A concurrent G change between discovery and recapture is a bounded read rejection before clock/exposure, not a final mutation CAS; after authoritative capture the native writer permits unrelated G advances. A3 noted this discovery rejection could eventually be avoided by discarding discovery entirely; the present A1-reviewed implementation keeps the bounded two-pass rule. Returned graph is explicitly dropped before any observer/publication await.

The first explicit discovery currently copies selected text before discarding it; native donor projection materializes namespace records. There is no constant-memory/constant-I/O or performance claim. Serialized reply admission is not a capture-RSS bound. Policy inputs are immutable QueryEngine fields; the same instance supplies capture, projection and observer inputs.

Replies retain exact full receipts/unknown IDs and suppress payload on failure. Missing UUID differs from a content-free terminal marker. True Full/Summary records exactly one direct exposure; false/missing/terminal skip the observer. Unknown work is not declared finished. Known roots finish before their real readiness barrier. No fake readiness, second registry, new profile or persistent codec is added.

## First bounded validation plan

No execution yet. Queue offered2CPU/4GiB/jobs1 after exact source/path handoff. Proposed fresh private root is /tmp/recalld-curie-o1-build, with cargo/ and target/; it is not either centrally released Curie path. No full cache clone. Use installed recalld-ci:1.94, RUSTUP_TOOLCHAIN=1.94.1, network none, source worktree only plus this private cache/output. No home/credential/socket mount.

1. Compile/run the ignored prepared_get_before_old_consumer_accepts_canonical_point_with_stale_vector probe explicitly. Expected RED is the actual unchanged old all-view getter rejecting an otherwise canonical point, not the public2 gate or a compile failure. Its executable/source will be this draft that contains the probe, NOT an asserted original19bd binary. Retain source/executable hashes and complete failure log.
2. Run prepared_get_ focused tests on the exact same source/executable, excluding that opt-in historical failure. They exercise actual tempstore MCP/REST parity, stale unused views, pending canonical admission, namespace/missing/budget rejection, one clock/event, original1/2 native/legacy writers, actual graph current/stale, terminal race, unrelatedG after capture, known publication failure/rebuild, and existing instance-local real postcommit unknown fault/reopen. Parser tests reject null bool/asOf/unknown fields and preserve namespace null as omission.
3. Format/compiler corrections produce a separately attributed source. No target/source switching or borrowed targets. If red/green source switching becomes needed, separate owned target or cargo clean -p recalld in this owned target before final rebuild; no other agent cache.

## Remaining before final proof/publication

- Native policy change with unchanged record token (where an actual valid writer can produce it), cancellation/close ordering, real canonical_applied+derived_dirty outbox admission, and complete boundary-size tests.
- The current64KiB single-exposure evidence reserve is a draft bound, NOT yet proved. Derive maximal typed exposure/state/history-event/receipt aliases and readiness/error encodings, exact escaping and private frame allowance; test each real transport frame. Do not treat existing query reserve as get proof.
- Shared daemon handoff is still unreleased. R1 is now on actualmain200; consume that validated parent only when its connection primitive is used. R2 is not a convenience dependency. Malformed-peer full received evidence must be retained without becoming attributable current-get commitment. No remote get or arbitrary outer request-ID framing guarantee yet.
- Appropriate frozen full/fmt/Clippy, exact-head CI and owner review after the complete actual consumer. No partial READY or public2/native startup claim.

Cleanup: no run/cache has yet been created; no bytes released/deleted. Sole central cleanup ownership remains unchanged.
