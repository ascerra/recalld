# Certified text compaction consumer

Validation complete; sealed for coordinating-agent review before publication.
No publication or activation claim. Runtime
`f908357369781950ffe369fb66622f74e14f0f2a` completed the full run below.
Test-only follow-up `13ef229e5dd9cd438e91591447f39d1e4a6654db` changes only
the obsolete parent-boundary assertion and its validation runner.
The only dependency merge is `c19ee17ea24fbd2da8cf3e89fc031a1b72e671d6`:
PR175 `7283a700c4eca5f8d1d8907b33c867b1d1e3a6bb` plus actual PR147
`50da71abee66cea159988a203d286f570f091718`, which already contains PR145.
No139/149/169 consumer or broader integration parent was added. PR175 is frozen.
Approved source plan: `54990c18`,
`docs/implementation/certified-native-consumer-plan.md` on the separate plan branch.

## Changed invariant

The existing selection transaction already checks the exact previous selection,
old/new reference digests, copied-text digest, complete logical-record digest,
and typed receipt. The new32-line dispatcher arm requires that specific typed
transition's source to equal the existing certification owner's generation, and
checks both descriptors and active selection in the same transaction. It then
uses175's existing prepare/commit/publish mechanism. This is not a generic outcome
allowlist, new generation authority, or new work registry.

The parent merge preserves both cutover reversal and physical relocation checks
in lifecycle history. Original foreign receipts remain bound by145's validated
restore anchor, with no new lineage version or rewritten history. Physical
selection changes canonicalG once but does not churn logical record/lifecycle/
policy revisions, document provenance, vector slots, or f32 bits. The internal
request remains compactText/application1 independently of manual1/autonomous2.
The selected payload is prepared and synced before one metadata transaction;
the protocol does not claim cross-file atomicity.

Known prior publication failures retain the actual prior receipt and allocate no
new physical mutation. Known post-selection failures retain the new receipt.
Unknown commit acknowledgement poisons the owner until strict reopen; no receipt
is guessed. The inherited worker-JoinError path remains RecoveryRequired/unknown
with no invented operationID, not a definitive NotCommitted result. No new panic
runtime claim is made for that projection.

## Actual candidates and failures

| Immutable run | Result | Classification |
| --- | --- | --- |
|before001|0 passed /1 failed,2.24s|Actual certified engine compaction rejects unsupported certification outcome; canonical control/G unchanged.|
|candidate002|31 passed /3 failed,21.68s|Typed dispatcher fixes original failure. Three old positive fixtures try fresh insertion without certification; no blanket fixture change.|
|candidate003|Compiler failure|New test used String conversion unsupported by Tag. Corrected to actual validated Tag constructor; no executable/runtime result.|
|candidate004|36 passed /3 failed,39.16s|New comparison helper incorrectly normalized live text offset to0 with nonzero length. Schema correctly rejects it. Corrected comparison to fixed valid48, preserving presence/length.|
|candidate005|39 passed /0 failed,52.14s|All focused compaction, certified mixed history and publication controls pass.|
|candidate006, f9083573|39 focused passed,52.18s; full1430 library passed /1 failed /2 ignored,864.78s;5CLI and10example tests passed|Only failure is175's explicit compaction-is-unsupported assertion at vector_certification/tests.rs1377. fmt0; all-feature/all-target Clippy0 with warnings,22.78s.|
|candidate007,13ef229e|Corrected boundary1 passed,2.83s;39 compaction passed,51.70s;fmt0/Clippy0 with warnings,22.77s|Test-only follow-up; no full-suite repeat or production change.|

The three candidate002 failures are owned-ID insertion and the two fork tests
sharing a fresh linked insertion. Only those named positive fixtures now call
the real fresh-destination reembedding helper. Common uncertified fixtures,
current-proof negatives, genuine BEFORE ZIP and old receipt bytes stay intact.
The candidate004 failures all occur in the same test comparison helper before
their respective assertions; they are not production corruption findings.

Candidate005 covers certified/uncertified mixed namespaces; exact encoded
logical records, retained documents and f32 bits; unchanged namespace configs;
strict logical export/import; zero added history scans for repeated captures;
seven actual certified process exits with two reopens each; unknown commit;
prior/new in-hand receipt publication failures and same-G repair; real manual1
and autonomous2 sweeps; two forks, phase loss, purge, foreign omissions and
malformed proof rejection. Existing cancellation test uses its original legacy
fixture. The new frozen candidate additionally verifies a real supplied native
insertion after compaction performs zero full certification/history scans.

The completed006 failure is preserved verbatim. Main reviewed its classification
and approved only a test-tail correction: actual typed relocation receipt lookup,
exactG+1, unchanged certification binding, and all four derived publications
matching the sameG after real acknowledgement. No production source changed
after006. Exact final-head CI must still run the full suite; a successful focused
tail is not relabeled as a full1431-pass run.

Pascal independently source-reviewed the32-line dispatcher and merged history
seam and found no blocker. That bounded review is not an independent test run.

## Reproduction and limits

`python3 docs/evidence/certified-text-compaction/run.py full LABEL` freezes source,
then uses Rust1.94.1 image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
offline Docker2CPU/4GiB/jobs2/testthreads2. There are no personal credentials,
data or socket mounts. One retained private target is package-cleaned before
each source switch. Source and executable manifests are captured even on error.
Default all-target runtime tests and all-feature all-target Clippy are distinct
claims; no all-feature runtime claim follows from lint compilation.

`focused-manifest.json` identifies preserved logs, source inputs, executable
hashes and source patches relative to the actual merge parent. candidate003
has no successfully linked candidate executable. OOM counters for candidate005
are zero; no resource-cap increase was used.

Ordinary native startup, publicAPI2 and A6 constructor activation remain outside
this leaf. Curie owns169 and139 consumers. A separate149 plan must consume actual
validated query guards before any constructor-wide profile restriction changes.
The query helper handoff is separate175-based commits
`3f76d77e47715499b1ec9180514fec8af2578018` and
`794506f6cec23d949fb600000690017ae085dcf4`, not a compaction parent.
Huygens owns the actual supplied/generated query consumer and its runtime proof.
Approved contingent A6 plan is `b57394b9`, with review/full-parent update
`d5158055`, `docs/implementation/certified-native-controls-plan.md` on the plan
branch. No A6 constructor source was changed here.

Completed raw175 runs were released centrally: all41 exact listed directories
deleted; the reported concurrent free-space delta547840000bytes is not exclusive
attribution. Both BEFORE inputs, source worktrees, shared registry and the one
active target remain. This leaf's raw runs stay protected until resulting PR and
recorded results; there is no additional raw HOME archive requirement.

## Full-source provenance

006 source-manifest SHA256
`9e77951e2ae0d84514af09814a78f3ea1ad2d989da341af6456782b6f3718dc5`;
all-executables manifest SHA256
`96233252f867274fafa445b71bcf49144463c1e12d5622e203291713a17de029`.
`full-manifest.json` hashes the full failed log, focused log, source/executables,
command and resource counters. Peak4294967296bytes, memory.max1921,
oom/oom_kill0. Peak is reclaim pressure, not an OOM failure. No resource increase.
HOME focused and final notes were appended to the existing actual experiment
and byte-verified. HOME-focused-manifest.json and HOME-final-manifest.json record
exact byte ranges and note hashes; the final note SHA256 is
`c5da4681eae75848b4773bae265518e748b5fd160df0e0b1e7b8383508585d51`.

## Final test-only verification and handoff

007 source-manifest SHA256 `75f5886d54de3312e8c111e12638c3a3cdb16399a7c49016de679fb5558cd9a6`;
executables-manifest SHA256 `3149a85e48f79954dfe5192a2d1e6ad7e088a896588117536d99bb796342f545`.
All 413 frozen source inputs matched the final worktree. The only source
difference from006 is `src/storage/vector_certification/tests.rs`, retained in
`test-only-followup.patch`. `tail-manifest.json` records exact commands/results.
007 peak4294967296bytes, memory.max956, oom/oom_kill0; no cap increase.
Clippy's1284 library-test warnings (1142duplicates) are disclosed, not clean lint.

The proposed PR base is175 with actual147 explicitly required. The narrow new
runtime file is `metadata/certification.rs`; named regression changes are in
`metadata/native_records/compaction_tests.rs` and `vector_certification/tests.rs`.
The parent merge also preserves175 prepare/publish hooks in the actual text
selection transaction and both parents' strict control/lifecycle-history arms.
No source/worktree/branch outside these owned leaves was changed. No new public
Rust API, CLI, schema version or serving flag was added by this compaction leaf.

All seven literal raw run roots and the single target are currently protected
pending publication. The completed Docker container has been removed.
After recorded PR/results, release only this leaf's seven literal run roots to
the central cleanup owner. Keep the tracked genuine fixture, code/Git, two old
BEFORE inputs and the one continuing target/shared registry. No raw HOME archive.
