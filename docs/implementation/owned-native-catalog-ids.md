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

Validation is in progress. Final runtime/source/binary/log hashes and exact
results will be sealed here before READY or publication.
