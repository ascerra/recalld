# Owned native catalog and policy allocation

## Scope and actual parents

This consumer joins actual PR124 `a5865da02969631a5031f65c6e43269701d7a254`
and PR134 `b62d1e2f4b3f516b5889946605946e274f82286c`. The merge is
`dbb1713`; it preserves the typed request context, strict historical validators,
known receipt failures, native recovery guards, and owned allocation introduced
by those parents. It does not enable ordinary native startup or public API2.

The three native catalog/numeric policy receipt calls must use the metadata
owner's existing `record_receipt` wrapper. This covers creation, changed policy,
and keyed equal-policy outcomes. The merge also passes the owned ID source to
`record_next_native`, the typed purge continuation introduced after PR124.
Reserved batch and purge child receipts keep their existing IDs; no second
allocator or replacement receipt format is introduced.

## Validation design and provenance

Halley prepared `docsbench/probes/native_catalog_owned_ids.rs` and the source-only
merge review in `docsbench/owned-native-catalog-ids-9e70229-review.md`. Feynman
registered the four actual API1/API2 regression groups in commit `133e952`,
with the three free receipt calls intentionally retained for an actual failing
baseline. The preparation document's "uncompiled" status describes that earlier
draft, not the registered tests. Evidence below must distinguish compile errors
from executed regression failures.

The tests begin after a genuine migrated native prefix. Only the future owned
allocation suffix is seeded; migration identity is not claimed deterministic.
An independent oracle restored from a copied cursor predicts operation IDs.
No live allocator is rewound. Closed-owner reopen restores that cursor once.

The production checks cover original API1/API2 hash identity, provider-free
replay and cross-contract conflict, one ID per new catalog/policy receipt,
receipted keyed no-change without namespace/revision churn, and no reservation
on replay, conflict or unkeyed equal policy. Fault checks retain the full known
receipt or uncertain operation identity and preserve an abandoned precommit ID
rather than reusing it. A purge regression checks start plus next-child
reservations, each actual deletion/cleanup transition, closed-owner reopen,
and no successor allocation at Complete.

Each run uses a frozen source snapshot and a separate private target. Only the
new target is package-cleaned after cloning dependencies. The source and
executables are hashed; failed candidates remain available. Containers use
Rust1.94.1, no network or credentials, two CPUs, 4GiB and at most two build/test
workers. Source and genuine PR134 BEFORE fixtures are mounted read-only.

## Boundaries

The existing A6 constructor/control/checkpoint consumer remains separately
owned. Ordinary native admission still requires the actual API1 writer, reader,
and adapter closure; public API2 additionally requires its independent A2
selection semantics. This allocator slice does not certify either gate.

## Executed results

Final tested source: `0a175afd9167e1147a3fc12b70eaa0fb055db75f`, tree
`3b3fa10e9577f7d1a2d774a7e7a0654250d8d769`. Production correction is
`73dd4c0410a1147fb40124783dd50f80e92df829`; the final commit changes only the
purge regression's startup-recovery expectation. Subsequent evidence/docs do
not require or claim another Rust execution.

| Frozen source | Actual result |
| --- | --- |
| `133e9526385cf29f590fff056a6e0d463c6f105f` | Baseline4 groups FAILED in1.52s: random operation IDs and owned cursor0 instead of1. |
| `73dd4c0410a1147fb40124783dd50f80e92df829` | Four catalog/policy groups PASS; added purge group FAILED in5.34s overall because its test incorrectly expected no allocations during pending-purge reopen. Full suite did not run. |
| `0a175afd9167e1147a3fc12b70eaa0fb055db75f` | Corrected5 groups PASS8.73s. Exact startup continuation uses two successors predicted from the pre-open oracle, verifies all3 historical typed child receipts, then exact cursor equality. Stepped API1/API2 paths and completed reopen/replay are also checked. |
| Same final source | Full **all-feature**1171 library tests PASS384.78s, plus2 actual CLI tests PASS1.43s/0.74s. All bin/example test targets pass. |
| Same final source | Focused filters: catalog13, policy8, purge21, owned-native producers8, IDs4, frozen controls3 PASS. Filters overlap and are not independent test totals. |
| Same final source | fmt and all-target/all-feature Clippy exit0. Clippy953 library/1019 test warnings (920 duplicates), plus1 binary warning reported in bin/test. No warning-free claim. |
| Same final source | OwnedEvaluator build and actual3-episode diagnostic with constructor negatives PASS; public legacyAPI1 stdio/HTTP-MCP/REST each3 fresh episodes PASS, no reported fixture violations;7 harness checks PASS. These are bounded synthetic diagnostics, not semantic quality or native/public2 activation. |

This child executes the all-feature suite. Its default-feature server is separately
built for the real public diagnostic, not a second default-feature full suite.
PR134's earlier1103+2 default suite and separate all-feature compile/lint evidence
remain distinct in that parent's immutable report.

Docker image ID is
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`.
Final cgroup peak is4012097536 bytes, with zero OOM/kill/max events. Source hashes
verify before and after execution. Each source uses a distinct target; only the
owned new target was package-cleaned. No switched-source stale executable proof.

Exact principal hashes:

- Executed final library binary: `e4e404818bc6c857309d1f4ef7b6c74d83222d8267881485f2204813171e3a7a`.
- Full suite raw log: `b46f2b4cf552004f2ad76c381a4b3be83f2c96c806ac39515f5304be65132901`.
- Clippy raw log: `f341a56579795e2267293cf5c62176ddfce12d57ec1a58443a52ca0f8a29d4a1`.
- Default server for public episodes: `3d0e27dfb68c50470381c493c866011682a25605d0bc97efdd4998c55d64c7d1`.
- OwnedEvaluator binary: `3cbf0b083dda62c81c98042741168c136d0f3bad0d9b8808f4cb905b6ed2ef28`.
- Evidence index: `f9ba093f40c0c8e3b8593eb3407d66ea9a9e6aca31c9fee691dbc8c71122002e`.

Raw compressed logs, source snapshots' hashes, executable manifests, actual
runner/harness, correction patches and synthetic reports are under
`docs/evidence/owned-native-catalog-ids`. Both failed candidates are retained.
The exact original received full receipt, unknown-operation identity, and allocation
cursor are printed in the focused fault log. No post-failure readback invents a
receipt. HOME verification and cleanup release are recorded separately at seal.
