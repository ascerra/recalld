# Agent resource cleanup

Caleb requires agents to clean up temporary resources when finished. His explicit retention policy (2026-11-14) takes precedence over older experiment instructions requiring raw-data archives.

- If an experiment is complete, produced a PR, and its results are recorded, delete its temporary data: datasets, databases, raw logs, binaries, copied sources and build caches. Caleb explicitly accepts losing this raw data. Preserve the PR/code and recorded results; do not delay cleanup to create or exhaustively verify raw archives.
- Keep active experiments and inputs still needed by another task protected. Preserve worktrees, Git metadata, branches, tracked/uncommitted source, recorded reports, personal recalld data and backups. A matching `recalld-*` name alone is not deletion authorization.
- Track exact owned scratch paths and containers. Before deletion verify ownership/completion, PR and results references, and absence of active process/container use. Do not remove a registered worktree or follow a symlink outside the authorized scratch root.
- During coordinated cleanup, release exact paths to the cleanup owner and stop using them. Do not race deletions or recreate released paths without a fresh ownership handoff. Do not use broad wildcard deletion or global Docker pruning.
- Under SSD pressure, pause new large builds and cache copies and prioritize deletion in bounded parallel batches. Avoid expensive size scans and redundant raw-data hashing when completion and recorded results are already established.
- Reuse a bounded private build cache where useful instead of cloning a full target for every run. Source changes still require correct rebuilding and executable provenance; stale binaries are never valid test evidence.
- Every completion handoff must report cleanup performed, approximate actual space recovered, and retained scratch paths with owner and reason. Clean completed runs before allocating another large run.
- Keep durable findings and compact reports inside this repository. Never upload build caches/raw archives merely to free disk space. Experiments without a completed PR still need an explicit retention/release decision before unique raw data is removed; their inactive regenerable build caches may be cleaned safely.
- Tests must remain isolated from personal recalld data, daemon sockets, credentials and backups.

# Local web UI

- When changing an API response used by the UI, regenerate UI types and update docs/ui/.
