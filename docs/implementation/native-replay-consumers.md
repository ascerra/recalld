# Retired namespace replay consumers

Ownership transferred from Pascal to Feynman on 2026-11-14 before edits. Pascal
confirmed no overlapping binding work and retains new exposure implementation.
Actual parents are published PR114/a03b92212cd6650cde3b1096a200bf1fd6922378,
PR110/ee0a9063823547d7e28f584aa4b16ac979e28105, and the shared PR111 binding
child6b94936 (runtime55bcf43, evidence5ae31ff). No PR103 wire dependency.

Feedback lookup must use the existing native mutation guard and unchanged redb
write transaction. Capture the original stable namespace ID's typed binding,
compare its actual GenerationRef with that transaction, reconstruct the existing
scope-name/ID/manual-kind/quality/context request hash, then replay before fresh
CAS, clock or policy. Missing or invalid binding retains the already-known full
receipt. A changed request remains an idempotency conflict.

The actual forget and feedback completion paths must check optional namespace
name assertions through that same binding authority. Same-name recreation under
a new ID cannot replace old identity. Fresh writes retain current record/name
assertions and lifecycle checks. Locks stay graph then storage; no sync lock
crosses await, and worker publication leases and accepted descendants remain.

Planned actual regressions: initial keyed feedback and forget, completed native
purge, replay with and without asserted original name, wrong-name rejection with
full receipt, recreated same name/new ID, reopen and changed request/lineage.
Replay must preserve old receipt/hash and current G, not sample clock/provider or
alter canonical bytes. Use the real completion consumer and real purge/catalog
writers on synthetic fixtures. Parent joins retain all native/legacy outcomes,
batch-child None plumbing, and legacy catalog accepted guard through the shared
publication function. Ordinary serving remains gated.

This plan is committed with the consumer implementation; helper validation alone
is not evidence that the consumer gap is closed. Record failures and final exact
source/binary/log hashes separately before publishing.

## Implemented consumer closure

The private lookup methods accept the optional namespace assertion from the
actual DerivedCompletion worker. Feedback captures the original ID binding under
its existing MutationGuard and unchanged redb transaction, compares current
GenerationRef, then reconstructs the unchanged historical request hash. Forget
keeps its ID-only hash and verifies an optional name using the same authority.
Both assert names only after matching replay; binding/name errors retain the full
known receipt. Workers no longer re-resolve a live name after lookup. Fresh
writes retain existing record/name checks. No public transport or startup gate
changed; Pascal owns the actual public feedback/forget wire companion.

Composition includes required production glue: legacy/native typed outcomes
remain in control/effects validation; native graph/maintenance/purge effects
explicitly carry BatchChild None; shared catalog publication retains accepted
mutation ownership and the full committed receipt. Both exact legacy and native
purge chunk variants bypass the active-namespace guard for their deletion work.
Other mutations still require an active namespace.

Actual77c2d5d first failed both probes before replay because the joined active
namespace guard omitted NativePurgeChunk.21f1136 fixes that narrow join, then
reproduces both intended replay failures: missing feedback retired-name hash
evidence and forget original-name mismatch with its matching receipt.611083c
fixes both,2/2green. b6011a2 adds changed payload/key conflicts and malformed
retained progress requiring the known full receipt and unchanged bytes/clock.

b6011a2 first full run1090PASS/2FAIL at547.46s is preserved. Both were inherited
fixture assumptions about full-receipt publication errors. The catalog fix is
only the first hunk from the coordinating-agent-supplied checkpoint2 fixture-boundaries-final
patch, with original path/hash retained. It verifies the exact returned receipt
against canonical lookup after reopen, preserving unresolved-work/owner checks.
The purge fixture separately proves the returned deletion-child receipt equals
pending.first and canonical retained receipt, while canonical progress is already
Complete. Repair and keyed retry return original final progress and clear the
outbox. b563b35 then passed1092library+2CLI and Clippy with warnings.

The requested actual four-view barrier positive in17c9d52 exposed a real runtime
gap: the owned current-pending reader accepted only legacy purge version1 and
rejected completed valid native version2 after successful repair. Its failure
source/log/binary are preserved; the positive assertion was not weakened.
Halley explicitly transferred the disjoint current-reader boundary to this join.
75bc809 accepts only known1|2 with a schema2 gate for native2 and the existing
start-kind/identity check. Completed native2 requires typed start/terminal proof,
row identity/sequence/cursor/reservation invariants, and equality with two actual
canonical receipt rows under their ORIGINAL store-ID/namespace keys. Active
progress stays pending; unknown/malformed rows fail closed. Full strict startup
remains the ancestry and count-chain authority; no lifetime receipt/history scan
or blanket current-fork equality is introduced.

Final actual tests include nine malformed progress cases plus schema1 rejection
without byte changes, canonical Complete versus pending child distinction,
broken-index four-view barrier rejection, repaired keyed retry without advancing
G or changing final progress, all four ready stamps at that G, and a completed
native purge carried through actual export/import/backup/restore-fork then the
real barrier. Main and Halley source-reviewed75bc809 with no blocker; this is explicitly
source-only, separate from the recorded Docker runtime validation.

Final counts and exact source/tree/binary/log hashes are in
../evidence/native-replay-consumers/README.md and final-manifest.json. Helper-only
completion is no longer claimed: both actual completion consumers participate.
Ordinary API1/native dispatch, exposure, FTS scoring correctness and overall
writer/read coherence remain separate startup requirements with assigned owners.
