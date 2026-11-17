# Frozen source and retained failures

Base32ab3b3f, source49a6689c: focused checks57 passed (9new/16public/23group/9actual tokenizer). Required binaries built offline with bench feature in91.120s,2CPU4GiB, peak4294967296 bytes, max4044, oom0. No broad Rust suite.

Before compilation: source002 export rejected two newly integrated schema assets;003 offlineCargo registry contained onlyCACHEDIR.TAG;004 command omitted the target's declaredbench feature. These are retained export/build-driver failures. Two host compatibility commands stopped at argument parsing and ran no checks. Exact schema assets were pinned; complete shared registry mounted read-only;005 same-source required-target build passed.

Actual006 source49a failed before actor/provider at capability handshake: current owned_evaluator returns checkpoint/clock/sweep capabilities, while old OWNED.Control expects the historical inspect/barrier-only contract. One failed/six unattempted roster retained; no scores. Source controls.rs125–148 and actual raw wire agree. Container retired in7.108s. Failed-source owned evaluator/scorer retained via APFS clones, not full target copies.

New C-only correction reuses SWEEP.control_capabilities(RESET.CONTROL_CAPABILITIES), RESET.checked_state and OWNED.checked_barrier. It validates the exact advertised object, captures actual runtime state at inspect, and consumes only inspect/barrier plus rejected negative controls. It does not call advance/snapshot/sweep or claim reset/general determinism. Manifest declares the actual legacy-checkpoint-sweep constructor profile and lifecycle-owned unadvanced clock. Old OWNED manifest/helper remains unchanged; its same-current-binary incompatibility is an explicit expected rejection rather than a fabricated positive compatibility result. The new combined consumer must pass on that actual constructor.

Ten focused new checks cover the correction, including exact old/unknown capability rejection and current inspect shape. Further actual results remain pending; all failures/protected raw inputs retained.
