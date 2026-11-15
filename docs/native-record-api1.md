# Native record writers retain the original API

This narrow child of published PR121 (`3fa91d4`) permits API1 context through the
actual native plain-insert and tombstone consumers. `MutationContext::validate`
already accepts only1|2. Removing the additional API2-only condition preserves
that originating API in the existing request hash rather than relabeling it.

The runtime change removes five lines in `storage/metadata/native_records.rs`.
Exact namespace checks, native ownership/control checks, publication guards,
staging, final transactions, effects, removal ledger and replay order remain
unchanged. No shared validator or retained-context type is introduced. API1 and
API2 requests using the same scoped key conflict; matching replay uses its
original expected generation and returns the full original receipt.

This branch consumes no new compound-context codec, purge/catalog implementation,
wire branch or broad validation join. It changes no batch behavior or hash/proof
format. Linked-store, standalone graph, exposure and compound original-API
propagation remain separate consumers. This is not public activation or a
whole-store CAS/readiness/startup-completion claim. Pascal can consume this
minimal record child for the actual forget coordinator without wire ancestry.

The [evidence report](evidence/native-record-api1/REPORT.md) records the genuine
BEFORE API2 fixture, API1 failures, exact tested source and offline validation.

API and physical format are independent in both directions. This change does
not establish API2 writes on legacy stores: their insertion, tombstone and batch
consumers still have separate API1 guards/origin reconstruction. Those require
actual original-context consumers and historical-proof compatibility, not a
blanket guard widening. Legacy catalog receipt methods already accept supplied
contexts; their convenience create path still correctly defaults to API1.
