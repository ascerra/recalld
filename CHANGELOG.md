# Changelog

## 2.0.0 — unreleased

### Database upgrades

- New databases use schema v2. Supported existing databases migrate automatically before the server starts accepting requests.
- Startup backs up the stopped database, migrates a separate copy, and commits only after runtime initialization succeeds. Failed or interrupted upgrades restore the original data when recovery can establish its identity and stop startup with an error. Committed upgrades are never rolled back over subsequent writes.
- Migration preserves existing memory IDs, namespaces, text, vectors and relationships. Older vectors retain unknown model provenance; migration neither guesses their model nor re-embeds them. Keep the existing embedding configuration.
- Schema conversion preserves the existing lifecycle policy. Retrieval, reinforcement and decay metadata are represented separately in the new schema.

- Docker startup relocates existing volume-root databases into a reserved child directory with persistent recovery state, keeping schema-upgrade backups inside the mounted volume.

### Correctness and interoperability

- Improved durable write receipts, retry handling, namespace operations, deletion and recovery across local MCP and HTTP adapters.
- Tightened validation for embedding profiles, structured labels, protocol fields and bounded responses.
- Improved coordination between canonical storage and derived search indexes, including startup recovery and concurrent operations.

### Upgrade notes

- Stop all processes using the database before upgrading. Allow additional disk space for backup, staging and recovery, and keep the automatic backup until the downgrade window has closed.
- Do not open an upgraded database with a 1.x binary. Downgrading requires restoring the complete pre-upgrade backup directory with the older binary and loses writes made after the upgrade.
- Backup ZIP restore requires matching major/minor versions. A 2.0 binary does not directly restore a 1.0 backup ZIP; JSON/JSONL import is not a ZIP restore bridge.
- The upgrade was exercised with a database created by the released v1.0.0 binary. This does not establish compatibility with every earlier release or damaged database.

See the [upgrade and recovery guide](docs/guide.md#upgrading-to-20) before upgrading. No retrieval-quality or general performance improvement is claimed for this release.
