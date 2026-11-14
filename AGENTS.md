# Agent resource cleanup

Caleb requires agents to clean up temporary resources as work completes. This applies to the coordinating agent and every sub-agent.

- Keep tests isolated from personal recalld data, daemon sockets, credentials and backups.
- Track the exact scratch directories, build targets and containers each task creates. Share active/protected paths with the coordinator and any cleanup agent.
- Before removing a completed run, retain the evidence needed to reproduce and assess it: commands, exact source/build identity, results, relevant failed attempts, and required raw artifacts. Follow the repository experiment instructions; verify retained copies with hashes. Do not preserve entire build trees as evidence.
- After evidence is verified and no process or container uses the paths, remove owned disposable Cargo/target copies, temporary fixture copies and completed containers. Reuse a bounded private build cache where practical instead of cloning a full cache for every run. Source changes still require correct rebuilding and executable provenance; never reuse stale binaries as evidence.
- Do not remove active worktrees, Git metadata, branches, uncommitted source, unique unarchived evidence, personal data or backups. A matching `recalld-*` name alone does not establish that a directory is safe to delete.
- During coordinated cleanup, release exact paths to the cleanup owner and stop using them. Do not race other agents' deletions or recreate released paths without a fresh ownership handoff. Check live mounts/processes before deletion; do not use broad wildcard deletion or global Docker pruning.
- Every completion handoff must report cleanup performed, approximate space reclaimed, and any retained scratch paths with an owner and reason. Before starting another large run, clean up completed disposable runs first.
- Record durable findings and compact reports inside this repository. Keep large raw experiment evidence in the verified work-notes artifact location; do not upload raw build caches or archives merely to free local space.
