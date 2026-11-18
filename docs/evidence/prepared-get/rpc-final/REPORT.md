# O1 prepared get RPC: locally validated, integration handoff

**Full all-features/all-targets passed:1764 library tests,10CLI tests,15example tests; zero failures.** Four historical library probes remain ignored. Final affected RPC tests:2PASS/0FAIL/0ignored in22.53s. Formatting and explicit include-file formatting exit0. All-feature/all-target Clippy exit0 **with warnings**, not warning-free. Doctests0PASS/0FAIL/4ignored. No exact-head remote CI or current-main integration is claimed by this local seal.

Validated source **`0e3338e31e6b9f0d7056e79537794a5c564e4911`**; production runtime **`1dd1d0f4878f1a06027867878bcf09ed708b62ec`**. Later source commits cedd/0e only correct/strengthen tests and retain evidence. Library executable SHA256 **`5dad71e7b6b1dbcbda5bb1aecabe9e88a603ffabc3e088f545f1351e9fc65415`**. All19 executable hashes and clean source were reverified unchanged after full validation. Library elapsed2156.28s; actual full command16:46:11–17:22:27UTC2026-11-17. These are validation durations, not product performance.

Actual parents remain core `41ba2e840fe0d9312990cb2d3c5957da49bb8216` and R1 main `200a1f10b9db52d8e5a758cab50c092ca2b07359`, joined at e1451717. No new semantic parents. Halley released exact runtime hunks after the1d9 diagnostic; Huygens/Feynman reviewed them. Huygens+Halley subsequently ACKed1dd runtime source. Halley released the old O1 assertion in shared prepared_query_tests; Huygens/Pascal reviewed terminal-vs-policy response semantics. No parent branch was changed.

## Actual consumer and safety boundary

The production RemoteSearchAdapter/get_contract and real private daemon dispatcher now consume the owned prepared point getter. Get-only capability negotiation is independent of ranked-query capability. Before-send refusal, explicit ping downgrade/upgrade, reconnect and no after-send retry retain their existing typed distinctions. Original API1|2, requested access and optional namespace reach the actual owner.

The private carrier is `{wireVersion:1,httpStatus,replyJson:string}`. The sender serializes GetReply directly, avoiding the demonstrated subnormal f32→Value spelling cycle. The receiver checks the outer compact frame and the inner **checked all-numeric normalization charge before acquiring any typed reply/receipt**. Every numeric token, including SHA-array integers, is charged up to32bytes; overflow, duplicate/escaped-equivalent keys, excess keys/depth/nodes and invalid finite numbers reject. This is bounded decode work/encoding, not an allocator/RSS or hard OS-deadline guarantee.

Whole typed JSON must reproduce the original inner bytes. Both receipt aliases must have identical canonical bytes, preserving signed-zero distinctions that FiniteF32 equality alone does not enforce. Native success also binds phase, createdAt and captured lastAccessedAt to the validated transition beforestate; it does not bind projected strength or invent generation adjacency. Legacy live-anchor clamp semantics remain distinct.

Named supported receipt fragments from an admitted malformed/crossed reply are retained as original parsed **unverified received evidence**, never current commitment/readiness. Unknown variants and invalid/oversized structural frames fail before evidence acquisition. Whole-get failure retains known receipt aliases and actual unknown operation identity; the dispatcher does not convert a post-observation outcome to a generic BridgeError. R1's existing Number-spelling check and Value receipt APIs remain unchanged; no global serde, stored codec, request hash or public routing change.

## Executed proof

- Actual temporary Unix-daemon point controls: false byte preservation; true one event/clock and full receipt/readiness; original1|2; missing/wrong namespace/terminal outcomes; refused origin/capacity before owner entry; cancelled client/accepted root completion; ping/reconnect/no repeated observation.
- Actual native subnormal-stability insertion through the canonical writer, then false/true RPC retrieval: preserved record and both receipt bits, actual operation lookup and one exposure. Native-before phase/timestamp mutations reject while preserving the original receipt as unverified evidence.
- Real captured-read races over RPC: unrelated canonical mutation advances G without recapture; deletion and policy replacement reject with no payload/exposure/receipt/readiness/unknown ID, retryablefalse, one captured clock, unchanged encoded post-writer record and canonical state. Terminal exposure keeps current coarse503/INDEX_NOT_READY/notCommitted; policy conflict remains409/LIFECYCLE_CONFLICT. The terminal label is not an assertion of an actual index defect or complete final error-taxonomy coverage.
- Real publication failure retains the exact receipt and permits repair; actual postcommit unknown preserves its operation ID and strict reopen reconciliation. Final frame refusal retains known receipts. These are task/process/reopen and injected-fault proofs, not power-loss experiments.
- Actual RPC independent inner-charge and quoted-frame F−1/F/F+1; malformed/duplicate/depth/key rejection before evidence; supported foreign BatchReserved and15000-outcome exposure evidence stay unverified and within public framing. These foreign/maximal-state inputs are explicitly encoding specimens, not invented committed history. Direct typed extreme/invalid-domain diagnostics remain separately attributed.
- Fixed completion bound is source-derived and guarded against emitted shape drift; see [typed carrier plan](../../../implementation/prepared-get-rpc/TYPED-JSON-PLAN.md). The diagnostic's pointer-map analyzer is not substituted for the production borrowed visitor. Get F=2774088, outer ceiling8388608, point reserve65536.

## Retained failures and exact attribution

| Source | Actual result / role |
| --- | --- |
| a2374ddf | Actual owned local get200 versus real daemon unknown-method BEFORE:1FAIL. Provisional frame allowance was later replaced; failure preceded limit parsing. |
| d76451c9 | First RPC focus28PASS/1FAIL/1ignored: old-peer test dropped its client early, causing EOF. |
|36e4915e | Corrected old-peer control1PASS; explicit ping downgrade1FAIL (one unexpected get sent). |
| ecfe4564 | Ping correction and prior RPC focus33PASS/1FAIL/1ignored. Extreme scalar OBJECT-carrier check failed; pinned serde diagnostic demonstrated a spelling cycle. Not claimed a reproduced persisted-store failure. |
|070fa8a1 | Direct typed GetReply/receipt-alias numeric diagnostic1PASS; actual features/source hashes pinned separately. |
|1d9fe7ba | Recovered completed build; joint-budget diagnostic1PASS in7.99s, executable5ca065ac. No rebuild before that run. |
|1dd1d0f4 | New runtime focus39PASS/2FAIL/1historical ignored in70.62s: obsolete raw `reply` fixture assertion and overstrict409 terminal-race assertion. Original logs/full error retained in [carrier focus](../rpc-carrier-focus/REPORT.md). New RPC limits, native subnormal and old scalar correction passed. |
| cedd0fd5 | Runtime unchanged; two corrected actual RPC tests passed in22.05s, executable5d2e33d7. Source and small proof included here as cedd-* files. |
|0e3338e3 | Only stronger negative race assertions added. Final affected2PASS, full1764/10/15, fmt/include-fmt/Clippy/docs results above, executable5dad71e7. |

Counts are not added across repeated tests. The earlier core1718 full run is its own source's proof, not this RPC composition's result. Earlier evidence directories remain intact.

## Resources, cleanup and remaining work

One existing private cache/target; recalld-ci:1.94 image `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`, toolchain1.94.1, networknone,2CPU/4GiB,jobs1/tests2. No home/credentials/socket mounts, downloads or personal store/daemon access. Actual image/limits/mounts are recorded in CONTAINER.json. Full-test cumulative peak844734464bytes; final checks including Clippy cumulative peak4228681728bytes; OOM/OOM-kill0. Build pressure is separately recorded. All owned containers retired normally via --rm. No cache clone or source-root switching.

No push, PR merge or deployment. READY here means local implementation/proof ready for integrator review, **not current-main or public activation clearance**. Halley must compose/review the exact current main, run appropriate joined checks and obtain exact-head CI before shipping. Ordinary public API1 get still uses its separate existing consumer path; its native point-owner dispatch remains the next actual Curie consumer task. No public2, ordinary native startup, whole-store CAS, general performance/quality, or complete A1–A6 claim.

Central cleanup remains sole owner.0bytes of protected host roots released in this handoff; ephemeral container space was not measured. `/tmp/recalld-curie-o1-build/{cargo,target,runs}` remains protected for unpublished O1 proof/integration and one reusable private cache. After PR/results are recorded, send exact completed run roots to the central owner; no new HOME archive is required. Preserve both registered worktrees and Git metadata. Do not recreate already released older roots.

[VALIDATION.json](VALIDATION.json), [EXECUTABLES.json](EXECUTABLES.json), [CHANGED-SOURCE.json](CHANGED-SOURCE.json), [SOURCE-AFTER.json](SOURCE-AFTER.json), full.log and the stage logs pin the actual execution. Clippy is a labeled compact excerpt; RAW-MANIFEST.json pins its complete protected original. SHA256.json seals this compact evidence package; raw build output stays in the protected run root.
