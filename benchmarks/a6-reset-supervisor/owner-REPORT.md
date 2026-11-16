# Typed logical hash diagnosis of independent namespace creation

Tests-only diagnostic source `2d69e53a0d7ea6f4dba943b0bbd44343cb1251e0`, branch
`study/checkpoint-hash-inputs`, direct parent PR115/`0750f13`. No production hash
or serializer change. `logical_export.rs` (before the diagnostic module),
`offline_snapshot.rs`, and `validation.rs` are identical to helper source `3ff4e31`.
This is evidence for the existing failed independent-run comparison, not a
shipping runtime prerequisite or a passing independent canonical replay claim.

## Exact result

The actual `visit_items_for(snapshot, true)` walker and `encoded` encoder
reproduced all four recorded hashes. The actual `LogicalHash` and
`isolated_legacy_hash` agreed. `compare.py` independently reconstructed each
SHA-256 from the emitted source encoding, domain and length-prefixed included
encoded items. No values, IDs, receipt references, ordering, or hash inputs were
normalized by this diagnostic.

Prefix: 34 typed items, 31 included by the existing logical hash policy.
Suffix: 41 typed items, 38 included. Exactly two included items differ in each:

* Item 1: namespace `foreign`, ID 2, `createdAt` 1794686998839 versus
  1794687070301.
* Item 10: `mutation_receipts`, operation
  `e9f0184f-13ac-4d5f-8d98-2a4b56fe7022`, committed generation 3;
  `/receipt/outcome/createdAt` has those same two timestamps. The encoded key,
  request hash, operation ID and all other receipt fields match.

Every other encoded item matches, including excluded catalog items. The exact
source GenerationRef is identical at generation 10 for prefixes and 14 for
suffixes, store `4507de41-5344-46b2-b5cb-43df21f436de`. Default namespace creation
is already controlled at 1767225600000. The foreign namespace takes wall time
from legacy `McpNamespaceAdapter::create_namespace` (`Utc::now`), then persists
that time in both canonical namespace and receipt. Provider endpoint ports differ
in checkpoint configuration but no profile/environment/port difference appears
in the typed hash stream. This is missing owned clock coverage, not a hash bug.

The baseline prefix hash is
`11f9b58c23812e27b9a0670a79bc726d69d1c4deb44f4d55a203870b57294809`, changed prefix
`2d088ba7751d9341e4427c0e87e56a30b27af3970665fa515fe1a11663b209ad`.
Baseline suffix `6b0d8dad3a265787af45b55ad74b1ad11c4cf73e626fa892cea2d0f652d2cb7b`,
changed suffix `5ad97378fe0847265cd669c06180920d8857232f5880c2fc575b04fe0a8747d5`.
See `typed-input-diff.json` and the complete encoded streams, including decoded
JSON receipt-value differences. Original failed assertions remain with the
helper run; each lineage's three exact suffix restores remain a separate result.

## Isolation and provenance

Input paths and manifest/file SHA-256 values are in `inputs.json`. The original
helper processes were joined. Only explicit `filesSha256` canonical inputs were
copied into private scratch, with source and destination hashes verified before
opening the copied roots. Original fixture files were never opened writable.
Strict `OfflineSnapshot::open` validates the copies. Input manifests are retained.

PASS: one ignored-by-default diagnostic test, explicitly invoked; formatting
check passes. Offline Docker image
`sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef`,
Rust 1.94.1, 2 CPUs, 4 GiB, no network/home/credentials/socket mounts. Private
cargo/target, package clean before this source build. Executed lib-test binary
SHA-256 `469474f0d6b743a2243c53bf9e978fc6835342a7d1d57c5417d33c5b9b0be991`.
The manifest's other cached binary was not executed. Cgroup oom/oom_kill both 0.
No full-suite or clippy claim for this nonshipping diagnostic. Initial source
`1b6ac45` failed compilation on diagnostic digest formatting; preserved logs and
manifest distinguish it from the successful corrected source.

## Remaining scope

PR115's isolated legacy checkpoint track does not control public namespace
creation time. It proves replay from an already captured prefix, not identical
independent initialization or arbitrary namespace-creating suffixes. The scoped
consumer may report exact per-lineage resets and its declared actor-level label
canary, retaining the failed independent canonical comparison. A separate actual
shared-clock constructor/adapter fix and real public regression are required
before re-enabling the independent canonical-hash gate. No synthetic fixture
substitution, clock-field removal, or receipt/hash normalization is acceptable.
