# N1 local catalog focused validation 003

Runtime/test source: `25ac7749d01678e89f09c9e37938aea30786b09a`, formatting-only after `828baf7775139b8b1a08c3713e54180c81ebd8f9`. 448 Cargo/source/test/example inputs, 17,038,514 bytes, verified unchanged after execution. No full-suite or activation claim.

Actual offline Docker Rust 1.94.1, aarch64, default stack, 2 CPUs / 4 GiB / one Cargo build job. Same protected private target, own-package clean before rebuild, no dependency-cache clone, credentials, personal data or daemon/socket mounts. Container `recalld-feynman-n1-local003` exited 0 and was auto-removed.

- Formatting check: exit 0.
- Default library test compilation: exit 0, 55.03 seconds, 72 warnings.
- `native_catalog` filter: **35 passed, 0 failed, 0 ignored**, 1,648 filtered out, 54.61 seconds. Eight new local tests plus 27 existing controls; not 35 independent protocol combinations.
- Executable SHA256: `a9b505493a778cd5b2d7644285661704cdb6bdbf2063b8cbf9ffb9da660a8b04`.
- Memory peak: 4,294,967,296 bytes. `memory.events max=4388`, OOM/kill/group-kill all zero. This was memory reclaim pressure, not OOM.

Actual new controls cover MCP legacy/modern success, original1 hash, direct unbounded-native rejection, full known/unknown fault frames and strict reopen; HTTP create/error bodies; actual 3.7145 emitter bytes plus independent canonical receipt decoding; oversized outer-ID refusal and legacy availability; exact encoded limits; custom candidate rollback before namespace staging with owned clock/ID draw oracle; actual foreign pending insertion outbox rejection; and caller cancellation retaining worker lease, one unresolved root and drain rejection until reopen.

Separately unresolved source finding: the current API1 worker receives a real producer result before returning, but records its receipt only after the join. A post-result tail panic could lose already-known evidence. This run contains no such injected probe and does not close that finding. A separate source-specific RED and correction are required before readiness. No E1 core, daemon RPC, O1 get, startup gate or public2 activation changed.

Raw root `/private/tmp/recalld-native-api1-admission-validation/003` stays protected before PR and follow-up review. No cleanup/deletion or push performed; sole protected target remains reused under queue ownership coordination. Compact records are retained here; no raw HOME archive was created.
