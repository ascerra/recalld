# Typed keyed native graph no-op evidence

Runtime `38035646bd7fa774d857126b7ef19eb964f1d20f` is a direct child of #97 (`292fb29`). It does not consume
#101 linked insertion, numeric-policy, maintenance, batch, or broad integration
history. Native serving activation and derived readiness remain separate.

## Behavior

A fresh keyed identical edge request now returns `NativeGraphUnchanged` with a
strict `NativeGraphNoChange` proof and a zero-action outbox in the same metadata
transaction. G advances once; edge attributes/timestamps, peer tokens/counts,
content/provenance and lifecycle fields remain unchanged. No clock sample or
physical edge transaction occurs. The empty keyed request also binds its outcome.
Unkeyed no-ops still return None without a write. Matching replay precedes current
capture and expected-generation checks; changed intent under the key conflicts.

The additive proof has exact existing-edge witnesses and before==after peer
commitments, explicit namespace/policy/lineage boundaries, and retained key
reservation. Strict all-origin policy/profile checks apply before active-origin
rewind. Zero-action outbox validation rejects every action field; active history
checks retained peer and edge state. No mutating proof, historical request hash or
unknown-outcome acceptance was weakened. Foreign receipts are validated but never
replayed as physical edges during restore. Later purge/deletion joins must restore
removed-edge history and validated retired namespace bindings explicitly.

## Actual validation

**19 focused tests, 945 library tests and 2 CLI tests passed** in offline Docker.
Formatting passed; clippy exited 0 with warnings (summaries in MANIFEST), not a
warnings-clean claim. Four doctests remain ignored. Runtime Rust1.94.1, 2 CPUs,
4GiB, 2 cargo jobs; private cargo/target copies, own package clean for each source
freeze; no home, credential or socket mounts. Source snapshots were immutable,
allowlisted, mounted read-only, and verified after execution against working
source. No serial-test workaround or shared writable target.

The 19 focused count includes inherited graph regressions and the child helper;
it is not 19 new independent cases. Five new tests cover empty/CAS/unkeyed
immutability; actual committed/unknown failures; process exits before metadata,
after metadata, and after canonical ack; six malformed witness/outbox/key cases;
and well-formed wrong-profile rejection after actual backup/fork. Existing tests
now require a durable no-op between writer A and writer B, replay A without undoing
B, and export/import/restore of the new receipt. Original files remain byte-identical
after malformed startup/export attempts. The unkeyed empty path is byte-immutable.

Run 1 failed compilation for a missing enum import in the physical edge guard;
no tests ran. Its source and log are preserved. Run 2 adds the import and is the
complete green proof. No unrecorded retry or global cache cleanup.

Final source-manifest SHA256: `95ee63a5c5e8350a733b19aff94d7c565d8ae38069e8ce539d92ff26575bef50`.
Final validation-log SHA256: `7c3081672aec54f32db717d5bb5a1df288173bb423861e84cde593a6584a4e9c`.
Per-source test/CLI binary hashes are in MANIFEST. Source-specific package cleanup
prevents executable reuse across /work snapshot changes.

## Retention and remaining scope

Exact source snapshots, scripts, logs, hashes and source patch are retained outside
Git at `/Users/caleb.evans/work-notes/work/Experiments/Artifacts/recalld/2026-11-14-native-graph-noop-3803564`; COPY-VERIFICATION.json records each copied member's size/hash.
Git contains source, this report and text manifests only. Container inspect records
remain in the private worktree; published metadata records only this run's resources.

This closes the standalone successful keyed no-op gap, not all A4/native readiness.
The owner still requires producer-derived completion at the receipt G. Full graph
and history scans remain; there is no performance or bounded-total-work claim.
Incremental publication is a later consumer of the actual #96 coherent registry,
with typed contiguous deltas, before/current-G guards and explicit per-component
completion; full rebuild remains the fallback.
