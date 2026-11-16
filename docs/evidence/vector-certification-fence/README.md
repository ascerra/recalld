# Existing required-feature fence: genuine old executable

Source-only audit branch `audit/native-vector-certification`, based on frozen
PR159b7179391. Test-driver sourceee783333 adds one cfg(test) child module and
one test; production Rust is unchanged from runtime3e5fd806. Subsequent plan/report
changes are documentation only. No codec/feature allowlist, certifier, operator
command or serving activation is implemented. No shipping helper PR is proposed.

Actual proposed fence: existing NamespaceSchemaV2.required_features contains
`vector-recipe-certification-v1`. The old validator rejects a nonempty list during
NamespaceConfig decode in disposable native preflight, before live metadata open.
The probe uses a sentinel without a new certificate; this proves old rejection,
not validity of an unimplemented new-format positive. Source ordering was checked;
no syscall-tracing claim is made.

The old executable is the already-validated published159 library-test executable
from runtime3e5fd80658e5d8ba2eec92a1c5e27bf8e45ae4a9, SHA256
`4d4cb1ff49af47f1cce157747d2250a98bb98a0063adcfaf43c2b785a052f10b`.
It was copied once from the retained agent target before cleaning recalld for the
new driver. It is mounted read-only, distinct from the new driver. No rebuilding
old source and calling that a genuine old executable; no stale test-cache reuse.

The valid control is created through real legacy insertion, schema/profile and
lifecycle migration, then explicit inherited-policy adoption. The old binary's
existing native-maintenance child reaches its before-metadata-commit crash hook,
exit91. A separately created matching fixture receives only the prospective
required feature under test ownership. Old startup rejects twice, exit101, with
`unsupported namespace schema/features`; every file inside the source store is
byte-identical before/after both attempts. The test distinguishes this from an
unrelated history/binding rejection.

Offline Docker Rust1.94.1,2CPU/4GiB/jobs2, no network, empty HOME, no credentials or
socket mounts. Reused one private target, cleaned only recalld before source swap.
Compile40.65s; **1PASS,0FAIL,2.95s**, fmt exit0, driver0. Peak4261629952bytes;
memory.events max0/oom0/oom_kill0. Source-after hash verification passes. This was
one targeted audit, not another full suite or a certification implementation test.
Exact driver source/executable/log hashes and the old executable provenance are
in manifest.json. The original159 full validation is separate.

Plan: ../../implementation/native-vector-certification-plan.md. Curie reviewed
actual producer demand/replay/document composition source boundaries. Pascal and
A4 subsequently agreed on the immutable restore-pair rejection and scoped
uncertified admission; the plan records those source-only ACKs, incremental
owned-generation validation and retained Summary input evidence. These doc-only
refinements do not change or rerun this probe. Coordinating implementation approval
is still pending. Before implementation, review the proposed namespace cutover,
real re-embedding remedy, bounded witness shape and shared dispatch ownership.
Before promotion, prove populated/empty new-format positives, post-purge old-code
rejection and selected-inventory restore behavior. This probe does not establish
any of those unimplemented cases, remote-model authenticity or a serving gate.
