# Native reserved batch children

Work in progress on actual PR82 (6f56624), PR80 (ca205aa), and PR83
(a6087ca), plus Pascal's frozen phase-time correction a69865b. No catalog,
maintenance, broad checkpoint or public-v2 activation parent is implied.

The existing `batch_schema=1` container and `mutation_batches` table now also
accept `BatchProgress.version=2`. Version1 rows, receipt identities, reserved
child IDs, hash domains, and omitted native-default encoding remain unchanged.
The additive request `mutationVersion=2` (`batchMutationVersion` on MCP) selects
native children on an already-owned private native engine. `resultVersion=1`
continues to select the batch response envelope; this is not API contract2 or a
public whole-store CAS promise. Ordinary schema2 opening remains disabled.
Legacy serialization omits mutationVersion=1 for older daemon compatibility.

No extra journal, generation or post-commit status write is added. Native insert
and forget pass the existing reserved child into the final record/effects tail;
record, ledger, receipt, effect, and manifest outcome commit in one transaction.
Parent reservation and later rejection bookkeeping are separate real commits,
including all-invalid batches. Pending payload-needed slots stay pending and
count in the same accepted-work registry's barrier scan. Roots precede
reservation/provider work and descendants survive closing admission. Readiness
requires the existing coordinator; errors preserve the current child receipt or
unknown operation ID. Process exit experiments do not establish power-loss safety.

## Identity and replay

CONTRACTS lines38–44 govern identity. A4 confirmed there is no frozen batch
canonicalizer to import. Fixed semantic defaults (empty arrays and
checkDuplicates=true) expand before caller hashing. Initial stability is a
mutable namespace-policy default, so caller intent distinguishes omission from
an explicit value; the manifest freezes its expansion before executing children.
Default namespace intent and explicit caller profile identity remain caller
meaning, independently of the namespace's frozen stable ID and bound profile.
Provider outputs and generated IDs/times never enter the caller hash.

`batch_caller_v2`, `batch_resolved_v2`, `batch_reserve_v2`, and
`batch_native_insert_v2` are explicit domains of the existing length-delimited
SHA256 algorithm (receipt requestHashVersion remains1). Native child identity
encodes expanded stability/difficulty once, exact original supplied f32 bits,
ordered arrays, caller timestamps and supplied-profile identity, plus the frozen
policy/profile binding. The latter is resolution evidence, not a provider result.
The same child hash helper runs during reservation, replay, and final commit;
normalizing the working vector never replaces caller bits. Existing single
native and legacy receipt hash formats are unchanged.

Matching parent replay precedes mutable resolution. Child replay precedes
capture/provider/staging; the final guard rechecks scope, policy and canonical
capture/edges. Names never rebind a reserved child to a new namespace incarnation.
The native runner uses PR82's explicit-link native writer; auxiliary legacy
autolink writes are not enabled on a native handle. Native structured labels
remain verbatim, while raw tags are validated without lossy conversion.

## Recovery and offline consumers

The shared history dispatcher consumes only exact GenerationRef/operation-ID
references returned by strict version2 manifest validation. It does not allowlist
bookkeeping variants or admit active post-schema version1 batch writes. Startup
checks complete parent/child receipt, scope, hash, origin, order and outcome
bindings. Private native startup permits strictly valid pending version2
manifests so callers can resubmit payload; offline snapshot/export still reject
unfinished work. Existing raw-table export/restoration codecs preserve completed
manifests; strict validators reject unknown versions/partial tables. Feynman owns
independent history/offline review. No live-copy or whole-core barrier guarantee
is added here.

## Validation

Pending final frozen-source results. Development logs include parent composition
compile fixes, five initial focused passes, and a test-only wrong process-hook
exit-code expectation (86 versus the existing hook's91). That failed log is
retained. This is not a reproduced storage failure or a claimed red/green fix.
