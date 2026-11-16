# Vector recipe certification: validated, awaiting publication review

Parent code is `a3c7010c50809bb231e220a7f4000dfd2802979e`, the actual159
`b71793910b0fc5593bb9644317a1f514ee6a303e` + actual158
`9c90f8201d476918077d3d08920fdb989269c205` join. Commit5ead7b24 contains the
reviewed format2 plan and the explicitly unvalidated earlier placement draft.
Validated source/test commit is `ae128766a2a4d34f1d5d0ad98d2e293ab1be78a8`. The final documentation seal changes no Rust source. Individual immutable run sources are
identified by `runs/*/source.json`; HEAD alone is not their runtime identity.
No ordinary native startup or API2 activation is enabled. Full candidate024 completed:1223 library pass/171fail/2ignored,4CLI+10examples pass,
fmt/all-feature-all-target Clippy exit0 with warnings. The complete
[failure matrix](FAILURE-MATRIX.md) separates five observed failure groups.
That frozen run remains unchanged. Subsequent runtime corrections and explicit positive-only fixture changes are recorded below; the common uncertified fixture and genuine history negatives remain unchanged.

## Changed strict invariant and actual fork RED

Remedy016 built the genuine tracked148 wrong-document fixture, performed actual
fresh-destination reembedding, backed it up and restored a serving fork, then
changed the migrated `LegacyInferred` row's summary without a writer receipt.
`OfflineSnapshot::open` accepted it. This was an actual WIP validator bug:
current-origin reverse history alone did not check foreign migrated rows against
the original cutover's retained document components. Existing immutable creation
provenance remained unknown, so the native-only creation checker did not cover it.

The fix uses the already validated cutover and original ancestry. Surviving
summary/tags/structured labels, label provenance, extensions and immutable profile
must match the original cutover commitments. Only validated removal-ledger losses
permit stripped components. Actual retained text is checked against the cutover's
explicit Present hash or Absent evidence, including legacy rows whose old
`ContentProof.text_sha256` remains None. No historical receipt, profile identity,
source ancestry or old provenance is rewritten. These scans are strict
inspection/reopen work, not query or insert work. Removing every trust anchor and
replacing the whole store is outside the authenticity guarantee.

Remedy018 passes negatives for foreign summary, tags, labels, extensions, changed
text and missing text, with rejected-source metadata unchanged. Its actual
fork→Summary→Ghost→hard-delete positive passes, including export/import at each
phase and unchanged original receipts/certification origin. Ghost vectors remain
retained until the existing deletion writer removes them with its ledger proof.

Exact016 source-manifest SHA256:
`01de3a94baa9730a12d8254e36ce7775ce063a9277be4f72fc032c67bd7b3515`.
Executable SHA256:
`d3c619f0ae659cd5a0c572c279ee5f037f3cae4cfe3907f9e01e23f7216856f0`.
[runs/remedy016](runs/remedy016/) contains the failed log, complete source hashes
and reconstructable runtime patch against a3c7010. The same base applies to
retained008/009 patches. Failed sources are not silently corrected in place.

## Receipt decode investigation

016 and018 also failed an overly broad comparison of one versus two receipt
decodes. Temporary test-only callsite tracing in018 identified **both calls** at
`src/storage/metadata/effects.rs:184`, the existing outbox-to-retained-receipt
consistency guard. The first insertion validates its new effect. The second
validates the first insertion's still canonical-applied, unacknowledged effect
and its own new effect. Twelve actual keyed policy no-op receipts add no decode
work. Both guards remain. The corrected regression asserts the exact pending
operation IDs, applied state and exact1/2 decode counts; repeated canonical
vector captures have zero decodes/full validations/certification scans. Supplied
UUID lifetime-history checking is an existing separate path and is not claimed
to be O(1). The temporary tracing is removed from runtime/test machinery after
preserving its observed callsites in018's log.

## Format2 and remedy boundaries

- Ordinary format1 serializes old None fields identically, without added nulls,
  and keeps its previous supported limits. Genuine148 format1 imports unchanged.
- Format2 is mandatory for any retained live or retired certification. Only
  current certified vector payloads carry a location hint; it conveys no trust.
  Final typed inventory/history/root and exact f32 hashes remain authoritative.
- The complete prepass checks ordering, cardinality, namespace dimensions,
  payload hashes, duplicate placements, checked sparse extent/growth rounding,
  footer/checksum/EOF before staging. New bounds apply to format2/certified
  reconstruction:100k namespaces/records,256MiB combined record/payload budget,
  256MiB aggregate rounded vector capacity. Canonical table prepass additionally
  bounds1m rows/512MiB; the existing16MiB line limit remains.
- Retired certification without live vectors remains format2 with no placements.
  Mixed certified/uncertified namespaces preserve uncertified provenance and
  f32 bits without deriving positions from old native peers' historical refs.
- Actual repeated process exits during reservation and reclamation, plus payload
  sync/selection/publication, resume without duplicate live ownership. Only
  unreferenced zero placeholders are freed. Subsequent allocation cannot replace
  a live peer. A validly checksummed, unique but wrong placement fails final
  typed proof validation; the unpublished stage remains unservable on retry.
- Genuine old159 parser rejects header2 before chunks. Deliberate header1 copies
  are separate invalid downgrade probes: unchanged new namespace shape rejects
  first; stripping the unknown optional field exposes the existing required
  feature fence before any Memory hint. Neither is called an ordinary old2 read.
- Actual CLI `reembed-vectors` uses the configured provider recipe, no embedding
  cache, an explicit full-CAS request and fresh destination. Exact matching
  destination/stage replay precedes provider configuration construction, source
  lookup or provider calls. The same canonical owner spans private inspection,
  provider work and final guarded publication. Summary uses only actual retained
  components; Absent text and LegacyRetainedOnly labels remain explicit.
- Known postcommit failures retain the full in-hand receipt. Unknown commit keeps
  its allocated operation ID and private stage; live semantic state becomes
  unavailable until strict owned reopen/replay. Neither claims derived readiness.
- Restore rejects a pre-cutover backup combined with a retained certified target
  before the first reconcile write/journal. Actual two-fork positives preserve
  original cutover origins and history; fresh old historical restore stays
  uncertified, not silently blessed.

## Actual validation so far

All runs use Rust1.94.1, offline Docker with network none,2CPU/4GiB,2 build jobs,
empty HOME, no personal daemon/data/credentials, one private target and read-only
shared registry. Immutable run roots are `/tmp/recalld-vector-certification-validation`.
Package output is cleaned before source-root changes. `run.py` records exact
source hashes, executable hashes, cgroup events/peak and command lines. No OOM
resource increase was used. Repository evidence has its own SHA256 manifest.

| Run | Actual result and attribution |
| --- | --- |
|008|1pass/1fail: actual remedy succeeded; logical import repositioned certified references and failed original creation proof.|
|009|2fail: rejected attempted historical-position inference displaced an unwitnessed legacy peer. Removed; not final format2.|
|016|8pass/2fail: actual foreign legacy summary tamper accepted; plus1vs2 receipt-count assertion.|
|017|9pass/1fail: first strict component fix; counter still failed. Initial diagnostic location was inside its closure, not an actual caller.|
|018|10pass/1fail: expanded tamper negatives and legitimate fork/loss/import positives pass; actual two callsites proven; old count assertion retained and failed.|
|019|13pass/1fail: crash/retry/hint tests pass; old downgraded probe expected feature error but earlier unknown shape correctly rejected.|
|020|Compile failure in deferred-provider generic coercion; no runtime tests claimed.|
|021|17focused pass; old-parser probes now distinct; mixed namespaces, retained Summary, unknown live commit and provider-free replay pass.|
|022|Actual CLI integration1pass; private generated input/profile/raw-output seal1pass. Real synthetic loopback HTTP adapter, network none. Retry succeeds with provider config absent and source moved.|
|023|17pass/1fail: catalog/graph/lifecycle/compaction-closed checks reached real derived setup; test omitted FTS directory and failed SQLite open. Directory fixture corrected only; no production change.|
|024|18focused pass(39.35s); full1223 library pass/171fail/2ignored(571.69s);4CLI+10examples pass. Fmt and all-feature/all-target Clippy exit0 with warnings. Exact failure matrix retained; notREADY.|

Earlier compile/setup failures and007 source-handle contention remain in original
run roots; see those logs. Small reports/logs are retained, not Cargo caches or
bulk raw-data archives.008/009 executable hashes were not captured at failure;
no replacement binary hash is attributed to them.016 onward records the actual
failed executable before the next package clean. This evidence is not READY for
publication until the final full result and any corrections are recorded.

## Remaining consumer closure

This159+158 parent has no later native physical-compaction protocol; its existing
native guard remains closed and must not be described as participating success.
The approved145/147 consumer must consume this proof state on its actual join.
Actual139/149 semantic-query/native-constructor consumers likewise need their
minimal validated join; public routing and ordinary startup remain gated. Curie's
169 selection-demand optimization and Huygens168 API1 batch consumer are separate
actual parents, not imported merely for coordination. Every new vector writer
must retain the private generated input/profile/raw-output seal or an explicit
supplied-profile assertion under the owned certification; callers cannot create
a seal by filling public vector/profile fields.

## Admission correction after024

026 actual BEFORE committed parentG9→10 with childdurableNotAttempted and transportUnknown; closed-source remedy rejected unfinished batch before provider, source unchanged. Final no-reservation assertion RED. 027 repeated only to capture genuine pending bytes; fixture ZIP SHA25666645065799277280ce613a30903189edda33ca3c0a67fa84d308443a1894859. 025 test accessor compile failure and028 test Debug-bound compile failure retained separately.

029 focused21PASS(42.55s): actual fresh supplied/generated single/batch reject before provider/clock/IDs/G/file changes, mixed-invalid admission, all-invalid bookkeeping followed by real remedy; genuine pending retains parent/child and nonfinal UNAVAILABLE without newG, unsupported remedy immutable; stale/inconsistent/missing owner remains RecoveryRequired; CommittedMutation retains its full in-hand receipt. This is focused evidence, not a corrected full-suite result. The complete024171 failure matrix remains retained and positive fixture corrections are owner-reviewed opt-ins only.

## Pending compatibility and owner-reviewed setup corrections

The approved concrete plan is [certification-pending-batch-consumer-plan.md](../../implementation/certification-pending-batch-consumer-plan.md), frozen SHA256 cf9055fa6fde75a202900f015adad7519644cd893f5dd77bcc215bfd7be2cd6f before implementation. Curie reviewed the batch seam and Huygens the error/operator boundary. Strict validation now supplies a private compact witness to the existing metadata owner: original full namespace hash, exact certificate-only expected hash, certificate identity and sorted original-store cutoffs. It has no serialized representation or independent generation/repair authority. The additional compact-witness heap is bounded to16MiB (not the already resident bounded history maps or total process heap) including construction/boxed-array overlap, origin arrays, allocation allowance and transient namespace/escaped-JSON capacity. The ordinary full schema/history validators remain required before collection.

A shared lookup/final-TX predicate compares actual canonical G, original parent store and cutoff, current entire config and original frozen policy hash. Native2/3 only can cross this exact cutover. A current config comparison is still performed on each call; carrying a witness across later G is not permission to ignore changes. Original request hashes, parent/child IDs, defaults and receipt history are unchanged. A private remedy inspection purpose permits strictly validated pending history across source, stage, publication and replay; ordinary export/backup/import/restore continue to reject unresolved batches. This is not native recovery or permission to repair source files.

Pascal supplied isolated source-only patches for four policy/replay corrections, seven A3 positive setup files, and three narrowly selected wire files. Hume supplied four graph/exact positive setup files. Their exact patch hashes, before/after manifests and reviews are retained under [owner-patches](owner-patches/). Feynman verified hashes and applied them, owns runtime/helper changes and all actual validation. The wire mixed test certifies only its first-loop Forget branch; feedback and FaultClock unknown-outcome branches retain original setup. Genuine old absent-binding replay requires neither adoption nor certification. Ghost structural tests retain old history; the separate semantic positive reaches Ghost through real post-certification phase loss. No helper-only or broad common-fixture blessing is claimed.

| Run | Actual result |
| --- | --- |
|030|Test-only Result-to-Option compile error in Hume's proposed Ghost setup; corrected after preserving failed source/log.|
|031|1pass/20fail; all20 traced to accidentally opening batch tables on the shadowed edges read transaction. Corrected to explicit metadata read; guards unchanged.|
|032|21focused pass43.09s, including genuine027 pending→real remedy→original child once→replay→logical import.|
|033|21pass/1fail51.24s. API2 supplied/generated A/B/A subcases passed; old159 binary rejected new API1 requestContext before hook. This was a BEFORE-binary mismatch, not bridge correctness evidence.|
|034|22focused pass56.32s. Actual159 API2 and159+158 API1 binaries created pending/partial A/B/A histories; sequential remedies preserve original IDs/prefix and replay.|
|035|237native-family pass/3fail/1ignored454.77s. All3 were fresh insertion paths in original wire setup; each classified, then Pascal reviewed explicit positive-only opt-ins. Intentional worker-panic tests passed and are not counted as failures.|
|036|Docker access denied by sandbox before any container/test; retained invocation only.|
|037|1focused pass25.37s across four genuine BEFORE cases, now including generated A commit while B remains uncertified/pending, followed by B remedy and original-ID completion. Four raw ZIP fixtures plus exact source/binary/hash provenance are tracked for CI; normal fixture-backed execution has no external-binary skip.|
|038|Test-only comparison attempted PartialEq on DiskRecord; compile failure, no runtime claim. Changed assertion to exact encoded canonical row bytes.|
|039|25focused pass/2fail52.62s. Runtime final policy guard rejected actual concurrent policy change without record insertion. Failure was expected replay response flag versus retained canonical flag. Precommit process-stage retry rejected without changing canonical files; assertion failed solely on first creation of empty compatibility lock. Both failures and exact sources retained; bounded test expectations corrected, no runtime weakening.|

A process exit before the cutover TX may leave a private unpublished stage with no matching receipt. Retry deliberately rejects that ambiguous stage instead of inventing a receipt, repairing it or deleting it. The original source remains usable and unchanged; the operator can choose a different fresh destination. Committed-stage retry instead must preserve its actual receipt and avoid new provider calls. This is process-exit evidence, not a power-loss durability claim.

Candidate040 passed28 focused certification tests59.02s,27 lifecycle wire tests30.86s and2 actual operator CLI tests1.54s; fmt0. This includes unknown-child reconciliation/fork history, compact-witness cutoff/config negatives, provider race, selected wire opt-ins and provider-free operator replay with pending parents. Candidate041 freezes this source plus one additional incremental-heap-cap boundary regression. Its final results are recorded below. No publication or ordinary activation is claimed.

Provenance note:040 records the library executable hashes, exact source and actual CLI logs, but the runner's filename filter missed the separately named CLI integration executable before the next package clean. No later hash is attributed to that040 executable. The final041 run captures its actual CLI/integration binaries before cleanup; the runner filter is corrected for future runs. This is an evidence-capture limitation, not a claimed runtime failure.

## Final exact-source validation and handoff

Runtime/test commit: `ae128766a2a4d34f1d5d0ad98d2e293ab1be78a8`. Actual dependencies remain159+158 via a3c7010;
no unrelated integration ancestry was imported. Candidate041 current source
matches its immutable source manifest exactly:
`008e473d09ec2a9c33ffd18d0a9fb58c406894b2fdbb4b536dc62bd2863c10d6`.

- Focused certification:29passed,0failed,59.53s, including the collector cap.
- Default all targets:1407library passed,0failed,2ignored,801.21s;5CLI tests and10example tests passed. Named CLI tests include both real operator paths.
- Fmt exit0. Clippy all-features/all-targets exit0 with warnings,25.42s. This is all-feature compile/lint evidence, not an all-feature runtime-test claim.
- Final driver0; cgroup peak4,294,967,296bytes, memory.max events1283, oom/oom_kill0. One2CPU/4GiB container and2buildjobs, no retry/resource increase. Run040 likewise reached4GiB with maxevents2146 and noOOM.
- All16actual test/operator/example executables were hashed before another package clean. Manifest `runs/candidate041/all-executables.json` SHA256 `6cc0310d8a222adc54b9638a8312db477866de7fc7588b5f7bc1b39adc2d84ad`. Library executable `3230578c2706150aa496d6cadf8ee6ebb59be031e13830e89c0b3615c011bcf8`; real CLI `3e2a736b45c7d505789435bc1dc6cff9e53424ae0879b9c9cce2fc43590fa9ac`.

Curie's source review f75ad7e and Huygens' direct bounded review are retained
under owner-patches with exact inspected hashes; neither is presented as an
independent execution of041. Pascal/Hume test patches retain their source-only
attribution and before/after hashes. All failed runs/classifications remain.

The actual HOME experiment is
`/Users/caleb.evans/work-notes/work/Experiments/Recalld correctness and retrieval research.md`.
Its entries record the preserved failures, exact source/executable provenance and
final results. No raw HOME archive was created. The single retained target and
active source/evidence roots stay protected until publication/results cleanup
handoff. No source/run-root cleanup or push occurred during validation.

Next dependent slice is the actual certification consumer join for native text
compaction145/147 and query/constructor139/149, using only real required parents.
It must preserve exact-G owner participation and per-namespace semantic gating;
metadata/FTS-only/structural access must not be silently disabled or certified.
Curie's selector/producers and Huygens' public adapters retain their own consumer
ownership. Ordinary startup/public API2 activation and whole-spec completion are
not established by this PR. Publication is held for the coordinator's exact-head
review, followed by READY/freeze handoff to Halley.
