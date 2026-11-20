# Changelog

## 2.1.1 — 2026-11-20

### Local web UI

- The memory map starts with memories active in the last 90 days, so large, densely linked stores lay out calmly. **Active in** widens it to 7 days, 30 days, 1 year or all time; memories outside the window leave the layout, not just the view.

### Fixed

- Decay sweeps on 2.x stores no longer stall for minutes. A startup or timer pass publishes derived views in batches of `write_batch_size` instead of after every memory (#220), and each commit reads only the memory's graph neighbourhood instead of every record and edge. Reads and writes are still refused while a pass runs, now for seconds rather than minutes on a store of about 1,200 memories. (#215)
- `recalld backup` no longer claims `-d` for `--destination`, so the global `-d/--data-dir` works with it. Spell out `--destination`. (#219)
- The guide no longer says the daemon shuts down after 30 idle minutes; since 2.1.0 it keeps running by default. (#218)
- A failed automatic 2.x upgrade no longer leaves another full copy of the database on every start attempt. When validation fails and the original is unchanged, the failure is recorded in `<data-directory>.recalld-upgrade-failed`; later starts on the same unchanged database with the same binary report the same error immediately without copying anything. Repairing or replacing the database, changing the recalld version or deleting that file retries the upgrade. (#216)

### Upgrade

- The legacy vector ownership repair now ships in the release binary as `recalld --config CONFIG repair-legacy-vectors plan|apply ...`, with the same arguments and checks as the former `repair_legacy_vectors` source example, which is removed. Release-binary users no longer need a Rust toolchain to repair a 1.0 database that fails the 2.0 upgrade (#217).
- When the automatic upgrade stops on a vector ownership mismatch (duplicate owner, freed slot still referenced, or missing slot), the startup error now ends by naming `recalld repair-legacy-vectors plan` and the "Repairing legacy vector ownership" section of `docs/guide.md`.

## 2.1.0 — 2026-11-20

### Local web UI

- Added `recalld ui`, a browser UI over your memories. It comes with the standard install and opens on `127.0.0.1`:
  - **Memory map:** every memory as a node, sized by its links and current strength and colored by stage. When an agent recalls, the map replays recalld's real spreading-activation path: seeds bloom, light travels along the links that were followed, results keep a ring, and memories suppressed by retrieval-induced forgetting cool briefly. New and forgotten memories animate in and out, and clicking a memory brings its links forward. A **Decay** scrubber projects every memory up to a year ahead with recalld's FSRS formula.
  - **Explore:** a fast, virtualized table of every memory with phase, strength, next phase change, namespace and last access. Filter by phase, tag, entity or agent, sort, and share the URL. Clicking a row opens it in place with its full text, labels, decay curve, activity and links.
  - **Timeline**, **Health** (counts by phase, decay forecast, storage, daemon status, connected agents) and **Search** (⌘K, recalld's own hybrid search with a score breakdown).
- Browsing never changes memory: every read uses recalld's read-only paths, so opening, searching or viewing a memory never reinforces it, resets its decay clock or triggers retrieval-induced forgetting. Only **Reinforce** and **Forget** (after a confirmation) write, through the same tool path agents use.
- Security: the UI binds `127.0.0.1` only, checks the `Host` header (`127.0.0.1`, `localhost`, `recalld.localhost`), exchanges a per-launch token for an `HttpOnly`, `SameSite=Strict` cookie, sends no CORS headers and requires a custom header on writes.
- The map pauses its layout and animation when the tab is hidden and stops entirely on other pages.
- Activity entries name the agent and query behind each recall. The daemon keeps a bounded in-memory journal of the last 1,000 recalls and reinforcements for this.

### Daemon

- The daemon no longer shuts itself down when idle. `recalld daemon start` defaults to `--idle-timeout 0` (never); pass minutes to bring an idle shutdown back.
- Fixed an overflow that made "no idle timeout" shut the daemon down at its first idle check whenever no client was connected. Auto-started daemons, which request no timeout, could exit about 30 seconds after the last client disconnected.
- Idle MCP sessions no longer exhaust the daemon's 32 connection slots. Connections without a complete next request within 60 seconds are reclaimed, clients reconnect before sending after 30 seconds idle, and a full daemon returns a bounded "unavailable" response instead of starting another daemon or falling back to the locked store.

### MCP

- `list_tags` now reports entities, topics and emotions for native memories. They were previously missing because native records keep structured labels separately from tags.

### Upgrade notes

- The standard release archives and `install.sh` now include the UI. The separate `recalld-ui-*` archives and `RECALLD_UI=1` are gone.
- Turn the UI off with `[ui] enabled = false` in the config or `RECALLD_UI_ENABLED=false`. Agents and the MCP server are unaffected.
- The Docker image is built without the UI (`--no-default-features`) and is otherwise unchanged. To build from source without it, use `cargo build --release --no-default-features`.
- From source, `make release` and `make install` now build the UI first, which needs Node 22.17+ and pnpm. A plain `cargo build` still works without Node; `recalld ui` then shows a page saying the UI wasn't built.
- Restart the daemon and MCP server processes after upgrading. A daemon started by an older binary keeps its old idle timeout and has no UI support until restarted (`recalld daemon stop`).
- Existing 2.0 databases need no migration.

## 2.0.3 — 2026-11-19

### Namespaces and memory moves

- Fixed namespace creation through daemon-backed MCP on native storage. The daemon now uses the existing bounded creation handler, checks response capacity before mutation, and preserves commit evidence on failure.
- Added `move_memories` for atomic moves of 1–100 Full or Summary memories between existing namespaces. Moves preserve IDs, retained text, labels, timestamps, lifecycle/access history, and graph edges. Embeddings are regenerated with the destination's configured provider and profile; compatible lifecycle revisions are required.
- Moves use an explicit store ID and stable idempotency key. Retrying the identical ordered request reconciles the original operation without copying/deleting records or repeating the move. Ghost/Tombstone memories and batches exceeding 16 MiB of retained full text are rejected.
- Added move-aware recovery, vector certification, history validation, and backup/restore handling. Preserved cross-namespace edges do not make namespace-scoped searches global.

### Upgrade notes

- Restart both the daemon and MCP server processes after upgrading, then refresh the client's tool list. Older daemons reject the new operations rather than falling back to unsafe behavior.
- Existing 2.0 databases need no additional startup schema migration. However, **after a move commits, older binaries cannot read its new durable move history**. Keep a compatible backup before using moves; downgrade requires restoring that backup and loses subsequent writes.
- Upgrading does not automatically move memories or create project namespaces. See the [move tool guide](docs/guide.md#move_memories) for requirements and retry behavior.

## 2.0.2 — 2026-11-18

### MCP usability and namespace guidance

- Added the read-only `get_session` tool, exposing the same connection defaults and namespace context as `recalld://session` to clients that primarily discover tools.
- Improved tool descriptions, agent prompts, and optional hooks: keep shared facts in `default`, reuse project namespaces for project-specific knowledge, and use topic namespaces when useful. Scope relevant recalls explicitly and avoid per-agent namespaces or unnecessary memory writes.
- Clarified access recording and duplicate detection: use `recordAccess:false` for inspection, give reinforcement deliberately, and treat similarity warnings as advisory rather than instructions to delete memories.
- Added recovery guidance for known transient read failures, uncertain writes, and old-client capability errors. Unsupported single-write retry keys are rejected before effects; durable batch resumption retains the original identity and unchanged full payload. Automatic retry policy is unchanged.

### Upgrade notes

- Existing 2.0 databases require no additional schema migration. Keep the existing database and embedding configuration.
- Restart the actual MCP server process using the updated binary, then reconnect the client and refresh its tool list to discover `get_session` and the updated descriptions. Reconnecting to an old process is insufficient; successful reads do not prove that an old client can write.

## 2.0.1 — 2026-11-18

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
