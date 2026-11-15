# Legacy namespace create publishes its actual generation

Direct parent main b0d19d1d40c5dcecb47cd4fa070ac13ba47b722c; no PR170 or native-policy parent.
The prior MCP path returned success after retaining NamespaceCreated at G3 while
all four derived views stayed at G2. The next empty recordAccess=false recall
failed coherence. BEFORE proof is in docs/evidence/legacy-catalog-completion.

MCP and HTTP now share the existing DerivedCompletion owner for legacy creation.
An accepted child retains the publication read lease through canonical commit and
handle publication; it releases that lease before the actual derived publishers
complete under the accepted root. Success shapes/defaults/options remain unchanged.
No generic stamp advancement, warmup query, readiness bypass or second registry.

Known publication failure returns the full retained NamespaceCreated receipt:
HTTP503 and MCP isError with INDEX_NOT_READY/commitState=committed/ready=false.
The existing daemon PartiallyApplied message carries the complete JSON evidence
unchanged; MCP emits it as JSON text plus negotiated structured content. An actual
lost acknowledgement keeps its operation ID and commitState=unknown with no
fabricated receipt. Worker loss has unknown identity. Cancelled callers cannot
release queued workers' leases or falsely finish unknown roots.

NamespaceRegistryAdapter::new(storage) remains available for reads. It no longer
supports create without real publishers: create rejects before work. Library
callers that create namespaces must use NamespaceRegistryAdapter::from_system.
The server's actual HTTP construction uses that method. This is an explicit
source-level compatibility correction for standalone storage-only constructors.
No public API2 activation or native catalog/profile behavior is introduced.

Read-only evidence distinguishes canonical persistence from SQLite coordination.
BundledSQLite3.49.1 can update fts.db-shm reader mark1 (104..108) during a read.
Tests retain strict canonical/receipt/vector, FTS database/WAL bytes and logical
FTS invariants; all other SHM bytes must remain identical. No whole-directory
byte-immutability claim is made. The initial001 failure and10-file inventory are
retained, including actual offset104 byte14→19 and pinned source identity.
