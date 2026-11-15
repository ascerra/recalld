# Native selection demand: independent consumer slice

2026-11-15. Parent PR158 `9c90f8201d476918077d3d08920fdb989269c205`.
Branch `impl/native-selection-demand`. No certification or activation claim.

Coordinator approved this bounded slice after source review. Feynman explicitly
ACKed ownership of `graph/native_selection.rs` and the capture-call arguments in
`derived_completion/native_store.rs` and `mcp/native_store_batch.rs`. His provider
seal substitutions in those caller files are disjoint; his metadata capture,
certification gate and final transaction remain his. Forward-port only the
validated minimal consumer hunks; do not import either agent's WIP.

The current capture always scans eligible namespace records and loads/hashes
their vector payloads. `FrozenInsertLinks` stores selected edges, temporal IDs
and times, and advisory scores; it requires no full candidate-score inventory.
Therefore the consumer can use the actual frozen `NativeStoreIntent` to avoid
unrequested work without changing the durable proof or historical hashes.

- Retain original insertion profile and numeric validation even with all
  auxiliary channels disabled.
- Semantic demand: autolink enabled and positive semantic cap, or advisory
  requested. Preserve vector width, normalization, slot/hash corruption checks.
- Entity/temporal-only demand: canonical labels/timestamps, scope/phase/caps and
  stable ordering; no candidate-vector load. Uncomputed similarity is `None`
  and must never produce semantic links or advisory matches.
- All auxiliary channels disabled: no namespace candidate scan. Retain actual
  generation checks and final transaction endpoint/policy/phase checks.
- Checked-empty advisory remains `Some(empty)`, disabled remains `None`.
- No codec/profile/hash/receipt changes, integrity bypass, new authority or
  performance claim beyond measured selection work counts.

Before runtime editing, add test-only selection-boundary read counters and real
single/batch probes on the old algorithm. Count actual calls to namespace scan
and candidate-vector load, not unrelated completion/rebuild reads. Exercise
off/entity/temporal/semantic/advisory modes, deterministic edges and original
matching replay. Retain the failing logs, source and executable identities.
After correction, add missing-score and corruption controls, real reopen/replay,
then focused and frozen full offline validation. Startup/offline validation and
actual derived completion stay unchanged; metadata-only selection is not a
global payload integrity scan.

Use one owned cache/target, network-none Docker Rust1.94.1, 2CPU/4GiB/jobs2.
This worktree's new `.validation` is active; old released scratch is never reused.
Central cleanup alone deletes completed raw runs after PR/results handoff.
