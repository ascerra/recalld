# Locked format-selection focused proof

Source `085a93a3fda52ea5bad4fdcb74c6fba2b38cf262`, based on exact
`b374f07e` + `6ebfd250` composition `5e6f163b`. Later uncommitted bounded-catalog
changes are **not** included in this run.

Four focused tests passed, zero failed or ignored, 1669 filtered, in 3.41 seconds.
Library compilation and full-tree fmt check exited 0. This is not a full-suite,
Clippy, public transport, or ordinary native activation result. Compilation
reported warnings; the raw log hash is recorded in `RESULTS.json`.

The tests exercise real schema migration for an empty namespace, legacy fresh
creation/reopen, and rejection of unknown control versions, partial control
tables, malformed schema, and nonempty roots missing metadata. A probe at the
existing full native preflight verifies unchanged source file hashes and both
the stable sibling and directory locks, before writable open. Successful detected
open preserves canonical control and namespaces and holds both locks until drop.
The ordinary legacy entry still rejects native schema; this test does not
authorize startup activation or certify any vectors.

The private detector adds one disposable metadata-copy inspection, reads only
table/header shape with constant enumeration state, and then runs the unchanged
full selected validator. No extra lifetime-receipt validation scan is added.
Temporary copy ownership ends before the full preflight's separate copy. The
current legacy text-recovery function only rejects artifacts; no source repair
or migration is attributed to it.

Run 2026-11-16 21:04:51–21:05:56 UTC: offline Rust 1.94.1, two CPUs, 4 GiB,
one Cargo job, one test thread, read-only source and root filesystem. No network,
home/credentials/daemon socket mounts. The existing private target was reused;
the previous package executable was hashed before a package-only clean, avoiding
source-switch reuse. All 446 source files (16,974,961 bytes) were verified after
execution. Executable SHA256:
`574de0d6bfa9cd1546d94dddaaedde37fee97a42344b02a0ec48db26b684b271`.

Cgroup peak was 4,294,967,296 bytes, with 7166 memory-max events and zero OOM or
OOM-kill events: reclaim pressure, not OOM. The owned container was removed and
its absence checked. The sole private cache remains protected. Raw run root
`/tmp/recalld-native-api1-admission-validation/001` is retained during active
implementation; central cleanup receives any later release. No raw HOME archive
was created.

Still required: actual bounded original-API1 catalog consumer, request-specific
transport admission and malformed-peer evidence policy, full route/startup
acceptance including semantic namespace policy, final composed validation and CI.
This private detector is part of that ongoing consumer slice, not a helper PR or
standalone completion claim.
