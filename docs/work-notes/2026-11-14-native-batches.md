# 2026-11-14 native reserved batch children

Question: can the existing ordered batch executor bind native insert/forget to
reserved child IDs, retain exact caller identity, and recover without a second
journal or an ordinary schema-v2 serving bypass?

Runtime checkpoint `010d2adb4d9bfe8681a923b75b551c1b15beb119` passed 12 focused
synthetic tests. Actual dependencies are PR82 `6f56624`, PR80 `ca205aa`, PR83
`a6087ca`, and frozen phase-time correction `a69865b`. No catalog/maintenance or
broad checkpoint parent was introduced. Feynman transferred batch ownership and
retains independent strict history/offline review. Pascal owns the separable
native disposable-recovery correction and standalone nonbatch regressions.

The implementation reuses manifest tables, reserved child IDs, canonical receipt
and effects commits, the accepted-work registry, and the existing coordinator.
The v1 hash/row encoding is unchanged. New version2 identity expands fixed
numeric defaults once, preserves original vector bits/ordered arrays and
caller selector intent, then binds the frozen namespace policy/profile.
Explicit native input uses verbatim labels instead of lossy compatibility tags.
Whole-store CAS, public contract2 activation, native catalog/maintenance/purge,
and complete coherent-view/barrier participation are not claimed.

The focused proof covers pending reopen, immutable replay after deletion,
same-transaction child receipt/outcome and generation accounting, corruption
rejection, original vector bits and default expansion, actual MCP/framed daemon
mixed outcomes, current publication-failure receipt and retry, generated invalid
output pending without child mutation, before/after metadata-commit process exit,
completed logical export/import and retained restore, and accepted native provider
work draining while admission closes.

Development evidence is retained, including two integration compile failures and
a wrong test expectation for the existing process-exit hook (expected86,
actual91). The later actual 9-pass/2-fail run found native preflight aborting redb
repair after process exit, plus the runner incorrectly calling the public
provider helper that intentionally blocks schema2. BEFORE source archive,
per-file SHA256s and exact executable SHA256 were captured before editing. The
fix recovers only disposable native inspection copies before strict validation,
and uses a private native generation helper sharing profile/dimension/numeric
checks while preserving the public serving gate. The corrected 11-case run and
12-case run with the additional close-race test passed. These are process-exit
experiments, not power-loss proof.

Validation uses `recalld-ci:1.94`, Rust1.94.1, offline Docker with `--network none`,
2CPUs/4GiB/two build jobs, a private copied cargo cache and target, and read-only
source for tests. The copied package artifacts were cleaned before the first
build. No BEFORE/AFTER source-root switching occurred. No home, credentials or
Docker socket were mounted. Only synthetic tempstores were exercised.

Final full-suite results, parent/source mapping and independent-review outcome
are recorded below and in the evidence manifest. The external work-notes copy
has its own hash-verified COPY-MANIFEST. No branch publication or deployment
has occurred.

Independent review found two further defects, reproduced in an immutable
0-pass/2-fail run: reversed runtime-error generations were admitted by the
manifest validator, and batch forget rejected UnknownLegacy profile metadata
although single native forget succeeded. The fixes validate ordered generations
across success and runtime error, and require a bound profile only for insert.
The corrected 14-case native run passed; the ordering regression also exercises
Error→Ok→Error. Both original corrupted bytes and valid offline roundtrips are
checked. The earlier full run was992 pass/2fail, with exact receipt assertion
adaptations needed after the parent join. Each corrected assertion passed alone.

A subsequent15-case focused run adds actual configured-options admission.
Native batch store with default autolinking, or requested duplicate advisory,
returns a pending UNAVAILABLE outcome before provider/child mutation, with an
explicit field. Parent reservation is allowed and advances generation once.
Completed keyed replay takes precedence over the new configuration gate. Positive
native fixtures explicitly disable unsupported options. No silent native store
semantics claim, no false child success, and no second journal.

Main requires future autolink intent to survive process exit and keyed replay.
WorkTicket alone cannot provide that durability. Hume owns a new additive
same-insert graph proof/outbox plan, Feynman its history review, and this owner the
producer/root integration. The guard remains until that actual solution is
validated. This is a design constraint, not a newly reproduced published bug.

Frozen final runtime `febe91310153f2caab9209ca4cfae0d9fbbc5844` merges actual
minimal recovery `fb804a0a72ccf7dde0a6ee7eaea2e91a6b7ba2db`; the only merge
conflict was an explanatory comment. Both native batch recovery admission and
Pascal's standalone native recovery tests/probe are preserved. Offline ordinary
inspection still aborts on repair. No catalog ancestry was added.

Feynman independently source-reviewed exact `febe913`/`cb3bd31` and reported no
blocker in ordered runtime errors, kind-specific profile validation, or the
replay-first auxiliary gate. He read both corruption-order probes and the real
UnknownLegacy daemon/offline test. He did not independently execute them; the
recorded test results are this owner's runs.

Final acceptance:1000 library tests +2CLI pass,4doctests ignored; fmt passes;
Clippy exits0 with694 library and761 library-test warnings (680duplicates).
The final library run took446.14s. Exact219-file before/after source manifests
match frozen runtime and actual test binaries are SHA256-pinned. See
[full evidence](../evidence/native-batches/README.md) and its manifest for every
retained log, source archive, binary hash and actual dependency mapping. The
intermediate guard binary/source capture straddled formatting and is explicitly
not claimed as an exact pair; the final frozen run supplies that exact proof.
