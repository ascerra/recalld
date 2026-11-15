# Performance entry guard: candidate and callsite audit

Hume, direct165. Candidate validation pending. Dependencies unchanged. This contains the application-level cold-registration defect; it does not correct upstream tracing, arbitrary callsites or establish the cause of historical c65 integration failure. The standalone raw-dependency probe retains its demonstrated defect.

Production change is restricted to `performance::target_enabled`: public static/global TRACE checks, then `NoSubscriber` inspection, followed by the original target-specific `enabled!` outside the dispatcher callback. `REQUEST` continues to take precedence. No sampling, context, clock, ownership, schema or canonical behavior change is intended.

## Complete performance-target audit

`rg` over every Rust source found one production target-enabled callsite, one shared event macro and three direct request spans, all in performance.rs. All other performance event sites use `performance::event!`; no direct target events or spans exist elsewhere. Filter strings in tests and module documentation are not emission sites.

| Site/path | Admission and context |
| --- | --- |
| Shared event macro | Checks `enabled()` before invoking tracing or evaluating fields. Without REQUEST it uses the guarded entry; inherited decisions are unchanged. |
| Three request spans | Constructed only when `decision.selected`; the root decision uses target_enabled, while nested requests inherit REQUEST. Unselected requests use Span::none. |
| Timer end/drop, Timeline mark/drop, receipts, sync_io, Hold drop | Emissions go through the shared event macro; disabled Timer/Timeline starts are None. Hold releases the real guard before reporting. |
| RequestDrop | Restores the selected span's captured dispatcher/span and REQUEST before dropping the pinned future. The unselected path retains its false decision. No new drop implementation. |
| Worker construction/run/queued timer | Captures dispatcher/span only if enabled; queued clock starts only with captured context. run restores REQUEST and selected dispatcher/span before worker work and queued completion. Unselected work retains the false decision. An unused queued timer's Drop uses the same gated event path. |
| Transport, provider, insertion, publication, storage, observation and search counters | Invoke only the shared macro; all cold event registrations are behind the common gate. Existing worker call sites wrap blocking closures in Worker::run. |

There is no uncovered production performance callsite requiring a scattered guard. This audit covers the actual165 source, not unrelated targets, future source joins or arbitrary external use of tracing. Existing tests verify captured workers/cancellation; new tests exercise cold scoped entry, global dispatch, untraced real MCP work, target-specific filtering, repeated scope entry/exit and disabled clock/allocation state.

Halley's exact daemon test handoff is `16885f96b1af2014b2e4fcaef04a23ed71176fdc`, two-file patch only, with test-only capture visibility. He reports actual fresh-child RED: successful framed store/canonical1/readiness followed by empty trace, exit101 and fmt pass. His original c65 failure remains unattributed. Our earlier actual MCP RED source/executable and standalone mechanism remain in the prior report/VALIDATION.

No production dummy subscriber, global serialization, per-request cache rebuild, changed interest declaration or dependency patch. Test-only subprocesses isolate process-global callsites without serializing the suite or mutating the parent's environment. No overhead/default-enable or general tracing fix claim.
