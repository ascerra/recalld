# Pending certification bridge: bounded source review

2026-11-15. Curie reviewed Feynman's live implementation against the approved
consumer plan SHA256 `cf9055fa6fde75a202900f015adad7519644cd893f5dd77bcc215bfd7be2cd6f`.
No owner source edits, builds or test executions. The dirty owner's HEAD remains
`5ead7b24a675db6b2d4125db0b7e9a4686a1acb5`; hashes below identify reviewed files,
not an executed or frozen release commit.

**No blocker found in the reviewed bridge paths.** This is source review, not
full implementation, runtime validation or public activation signoff.

- `PendingBindings` stores namespace ID, old/expected-full-config hashes, exact
  certification and sorted source cutoffs. It does not retain record arrays,
  payloads or duplicate namespace configurations. The strict history validator
  creates it only after full validation; ordinary serialized summaries do not
  gain a second authority.
- `check_policy` retains exact old hash behavior, uses the exception only for
  native2/3 and requires active namespace admission. Native lookup and final
  insertion/Forget transactions call it after original `replay_child` checks.
  The owner checks actual G and current-store parent before indexed witness
  lookup. Full current namespace hash plus selected certificate must match the
  original certification-only delta; unrelated policy changes cannot reuse it.
- Witnesses remain immutable historical facts as participating G transitions
  advance the same owner. Every use rechecks current config/certificate and G;
  policy changes invalidate applicability, purge cannot supply an active target,
  and unknown/failed publication retains failclosed behavior. This does not
  certify unrelated unsupported writer paths.
- `VectorRemedy` is distinct from native recovery in strict inspection. Source,
  staged publication and existing stage/destination retry use the private path.
  Ordinary offline inspection still rejects pending batches; remedy rejects
  unfinished purge/effects and invalid or foreign pending bindings. The legacy
  `recover_text_compaction` call only rejects markers, without repairing source.
- Actual A/B/A test source includes first remedy, actual committed A prefix,
  still-pending B, second remedy, original reserved IDs, later original-payload
  completion and zero new full certification scans. Cutoff/config negatives and
  provider-latched final-policy changes exercise the real consumer boundaries.

## Bounds and evidence follow-up

The16MiB checked budget estimates incremental compact witness storage plus
construction/boxing overlap and transient namespace/hash buffers. It is **not**
a bound on all bootstrap/process heap: existing validated cutover/history maps
remain resident while the collector runs. Keep that distinction in reporting.
The collector charges origins and fixed binding storage before allocation and
accounts for temporary strings/escaped JSON; its private construction depends
on prior closed-schema and sorted-origin validation. Requested a focused
budget-boundary rejection proof; none was found in the inspected pending tests.

Feynman reports03221focusPASS,03422PASS and037four genuine BEFORE-artifact A/B/A
combinations passing.039 reached actual policy mutation, final INVALID_INPUT
bookkeeping and no child receipt, then failed a test comparing replayed response
metadata with canonical `replayed=false`. Correcting that test expectation is
consistent with retained receipt semantics; original failure and exact full
receipt assertions must remain. These are owner-reported runs, not executions
independently verified by this reviewer. The final frozen suite is still pending.

## Reviewed SHA256

Paths relative to `src/storage` in the owner's worktree:

```text
e2d2a43d8738e6c2d1d04c6f4634ffa08de67b89633446b270ba0d6d8ef69a6a  vector_certification/pending.rs
58cee6823a00234828f950ddadc9253baae16fa09205eb5209e87f4b2deebf0e  metadata/batch.rs
0923aa72cf6f95a0c617c2b8c550743179bdfbdfbd7ed0445da5ff7669e3d128  metadata/certification.rs
8ff06ace907e04102f1cc357ec6713149ae2ac17be78f89427e4e3de1be149a2  vector_certification/history.rs
38a56c8059c1e61be193ed4e967831ef895f4e59978755de9dd75600c3ea2020  validation.rs
9299d1bceba222907accb347e195eb2864df70f75845339a019435ce3ca9575c  offline_snapshot.rs
1d6fe847c08320cd8a2770fd75fdb8394e5bb045f9419ee69403e7795846558b  vector_certification/offline.rs
081c03b57ec0343e9434ef6877b14e4b8214498dcb67fbdf00565276e084bdd6  vector_certification/pending_tests.rs
```

No shared edits or new raw artifacts. PR169 remains frozen with Halley; this
review is on the separate assigned documentation branch.
