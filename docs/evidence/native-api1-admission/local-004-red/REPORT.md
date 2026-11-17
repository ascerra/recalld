# N1 post-result worker-loss RED 004

Exact source `96f29f2aea7a3e6712ff6c199cdd642be5f887dd`: only cfg(test) injection and regression added to the previously focused-green local implementation. Production receipt handoff was unchanged. This is a new local N1 candidate finding, not the solved historical 9ed batch finding.

Offline Docker Rust 1.94.1/aarch64/default stack, 2 CPUs, 4 GiB, one Cargo job, same protected target with own-package clean. Formatting and compilation exited 0; compile 51.38 seconds / 72 warnings. The single aggregate test **failed: 0 passed, 1 failed**, 3.47 seconds, after traversing all four cases.

1. Before any producer result: unknown, no receipt or invented ID; canonical generation unchanged. Control passed.
2. After actual successful producer result: hook logged the full actual receipt; response omitted it.
3. After actual committed producer error at `before_handle_publish`: hook logged the full actual error receipt; response omitted it.
4. After actual `CommitOutcomeUnknown`: hook logged the allocated operation ID; response replaced it with null.

The test independently reopened each owner and checked the canonical operation against a seeded ID oracle. The two known-result cases require a full separate known receipt and unknown state without deriving an unknown-operation ID from that receipt. The actual unknown-result case requires its already-returned allocated ID but no fabricated receipt. Exact unresolved ticket counts (2 before/success, 3 after unavailable native mutation guard) passed. Final aggregate assertion records all three evidence losses; no assertions were relaxed.

Executable SHA256 `8a091e417d2632124d772c041db2c83938bb94f8b7060bf173634851dbc403dd`; source input manifest verified after execution. Failed executable hard-linked into the protected raw root before the next own-package clean; exact hash verified. Memory peak 4 GiB, max events 6618, OOM/kill/group-kill zero. Container exited 101 and auto-removed.

Required correction: hand actual returned success/error evidence to the existing collector inside the worker before its fallible tail; use only that collector on join loss, never readback/candidate/invented identity. Preserve unknown precedence and unresolved work. No correction or GREEN claimed here. Raw `/private/tmp/recalld-native-api1-admission-validation/004` remains protected, including failed executable; no raw HOME archive, cleanup or push.
