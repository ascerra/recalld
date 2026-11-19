# Changelog

## 2.0.1 — unreleased

### Legacy database recovery

- Startup errors now distinguish duplicate vector owners, references to freed slots, and missing vector slots, including the affected record and namespace identifiers.
- Added an opt-in, source-built offline repair example for legacy databases with invalid vector ownership. It requires an exact snapshot and explicit record-ID approval, regenerates recoverable embeddings through the configured local Ollama provider, and removes only approved records without retained text. It preserves the source, creates a backup and separate validated candidate, and clears derived full-text indexes for rebuilding. It never activates the candidate automatically.
- Existing 2.0 databases require no additional schema migration. Automatic upgrades remain strict: startup does not silently repair or discard records. See the [legacy vector recovery instructions](docs/guide.md#repairing-legacy-vector-ownership).

### Build and test fixes

- Fixed macOS path-alias assumptions in container-layout tests and updated the SQLite compatibility assertion for the bundled SQLite version.
- Pinned CI and release jobs to Rust 1.94.1 so a rolling `stable` update cannot give parallel jobs different compilers. Compiler provenance and test coverage checks remain enabled.

## 2.0.0 — 2026-11-18

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

### Dependencies and build checks

- Updated ZIP, bundled SQLite/zlib, and selected runtime/network dependencies to address known security advisories. ZIP archive validation and existing backup fixtures remain covered by compatibility tests.
- CI tests are grouped by function: Storage & recovery, API & protocols, Search & embeddings, and Runtime & lifecycle, on both Linux and macOS. Aggregate checks verify complete, non-overlapping coverage.
- Release and container builds use the committed dependency lockfile.

### Upgrade notes

- Stop all processes using the database before upgrading. Allow additional disk space for backup, staging and recovery, and keep the automatic backup until the downgrade window has closed.
- Do not open an upgraded database with a 1.x binary. Downgrading requires restoring the complete pre-upgrade backup directory with the older binary and loses writes made after the upgrade.
- Backup ZIP restore requires matching major/minor versions. A 2.0 binary does not directly restore a 1.0 backup ZIP; JSON/JSONL import is not a ZIP restore bridge.
- The upgrade was exercised with a database created by the released v1.0.0 binary. This does not establish compatibility with every earlier release or damaged database.

See the [upgrade and recovery guide](docs/guide.md#upgrading-to-20) before upgrading. No retrieval-quality or general performance improvement is claimed for this release.
