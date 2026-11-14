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
