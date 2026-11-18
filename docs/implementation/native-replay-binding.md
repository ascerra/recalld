# Shared checked namespace binding for replay

This private backend child of PR111 implements the agreed interface for the
actual Pascal/Huygens replay consumers. It is not an ordinary serving gate or a
claim that their after-purge replay adapters have already joined it.

`MetadataStore::capture_namespace_binding(&MutationGuard, NamespaceId)` returns
`Result<(GenerationRef, Option<NativeNamespaceProof>), StorageError>`. It checks
the existing guard, opens one read transaction and reuses native_control_at for
the owned native handle's availability/schema/lineage checks. It never reacquires
publication. A caller with an unchanged write transaction compares the returned
current GenerationRef to that transaction's control before preparing the original
hash. This G is not the historical receipt G or a readiness certificate.

A live namespace returns its actual typed config witness. Strict native open
establishes live/retired agreement and participating writers preserve it; unknown
outcomes poison the owning handle. A missing live namespace falls back only to
purge::validate_existing and the shared borrowed retired_namespace_bindings
helper, also consumed by strict lifecycle history. Conflicting witnesses fail.
Only the selected existing witness is cloned into the caller's owned result.
No raw parser, stored format, retry hash, receipt, name or provider evidence is
invented. Old version1 purge progress has no native binding proof and supplies
no fallback. Unknown legacy profile provenance stays unknown.

The witness includes immutable ID/name/dimension and schema embedding binding,
plus original configuration evidence. It is for retry identity/assertions, not
fresh policy authorization. Callers retain their already-known full receipt on
capture errors; absence is None, corruption is Err. A recreated same name has a
new ID and cannot replace the original. Consumers must release guards before
exclusive awaited completion and reject stale caller lineage before replay.

23focused checks passed:3new capture regressions plus1existing binding test,
5mixed-history and14purge regressions. Tests cover an already-held mutation guard
and unchanged redb write transaction (no lock reacquisition/deadlock, exact G),
live→purge→same-name recreation/reopen, foreign guards, missing identity without
name guessing, no clock/generation changes from capture, and unknown profile
retention. All-target Clippy exited0 with warnings. The parent's full1001+2 proof
remains distinct; this child does not claim a new full-library run.

Pascal owns actual feedback/forget replay hash/name assertions; Huygens owns any
policy name assertion consumer that needs the dependency. Their current96-only
slices remain fail-closed with a known receipt until the real consumer join.
No standalone helper-only PR is needed; publish the backend with its actual
consumer and test original key→purge→same-name/new-ID replay without fresh
provider/clock/mutation. Ordinary native activation still requires that closure
and the API1/native-schema dispatch matrix, preserving independent API/disk/
lifecycle versions and existing API1 payload/default behavior.
