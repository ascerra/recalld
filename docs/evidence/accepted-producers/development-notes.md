# Development history and provenance limits

The final acceptance proof is the read-only frozen-source run, with source hashes
before/after and hashes of its actual test executables. This worktree kept a
private source-specific target; the initially copied package outputs were cleaned
before its first build. There was no BEFORE/AFTER source-root switching.

The retained `parent-join.log` records the initial compile failures when reconciling
the two real parents: scoped-forget test arity, plain publication guards versus
`MutationGuard` in observation preparation, and exhaustive batch outcome arms.
Those are integration compile failures, not a red/green storage experiment.

Other intermediate logs were reused during development. Their original full
failure output and source/binary hashes were not archived. The following is a
transcript excerpt, not a reconstructed raw log or acceptance proof:

```
producer_work_tests::standalone_graph_discovery_and_explicit_edges_finish_after_close ... FAILED
assertion `left == right` failed
  left: 0
 right: 1
test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 897 filtered out
```

That assertion incorrectly expected a peer-edge count for a temporal edge.
The final test asserts the canonical persisted edge type and the published graph;
existing edge-count tests remain in the full suite. No runtime defect fix is
attributed to that test-expectation correction. Earlier transient compile/format
failures also included a missing trait import and a formatter reading a file
while its host write was incomplete. Only surviving logs are included.

The pre-hold 11-test focused log belongs to an uncommitted development source.
The post-hold focused log covers all eight producer integration tests, including
three later review additions. Final full validation includes all fourteen new
regressions together; only its source/binary mapping is the release proof.

All stores/providers are synthetic and isolated. Process-exit/reopen tests do not
establish power-loss durability. No benchmark or performance conclusion is made.

## Final script mismatch

`final.log` retains the complete successful all-features run (906 library and two
CLI tests, four ignored doctests), followed by `no example target named
\`offline_eval\``. The wrapper exited 101 at that command. PR70+78 has no such
example. No unrelated parent was added and no passed suite was rerun. Required
format and Clippy checks ran separately on the same frozen source; their log is
`remaining-checks.log`. This is a validation-script mismatch, not a failing test.
