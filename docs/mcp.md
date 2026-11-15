# MCP Integration Guide

## Setup

### Claude Code

Two transport options: stdio (recalld launches as a subprocess) or HTTP (connect to a running server or Docker container).

**Stdio transport** (default, launches recalld as a subprocess):

```sh
# Global (available in all projects)
claude mcp add --scope user recalld -- recalld mcp

# Or project-only
claude mcp add --scope project recalld -- recalld mcp
```

If recalld is not on your PATH, use the full path:

```sh
claude mcp add --scope user recalld -- /path/to/recalld mcp
```

**HTTP transport** (connect to a running `recalld serve` instance or Docker container):

```sh
claude mcp add --scope user --transport http recalld http://localhost:7680/mcp
```

Use the HTTP transport when running recalld in Docker, on a remote server, or as a shared service.

#### Allowing MCP tool permissions

By default, Claude Code will prompt you for approval each time an MCP tool is called. To allow recalld tools automatically, add them to your `~/.claude/settings.local.json` (global) or project `.claude/settings.local.json`:

```json
{
  "permissions": {
    "allow": [
      "mcp__recalld__store_memory",
      "mcp__recalld__store_memories",
      "mcp__recalld__recall_memories",
      "mcp__recalld__get_memory",
      "mcp__recalld__reinforce_memory",
      "mcp__recalld__forget_memory",
      "mcp__recalld__forget_memories",
      "mcp__recalld__find_similar_memories",
      "mcp__recalld__create_namespace",
      "mcp__recalld__namespace_stats",
      "mcp__recalld__list_memories",
      "mcp__recalld__list_namespaces",
      "mcp__recalld__list_tags"
    ]
  }
}
```

The `permissions.allow` array merges across global and project-level settings files, so you can allow the MCP tools globally and they'll work in every project.

To set a custom default namespace for a project, create a `.recalld.toml` file in the project root:

```toml
namespace = "my-project"
```

When recalld starts in MCP mode, it walks up from the current working directory to find the nearest `.recalld.toml`. The `namespace` field sets the default namespace for scoped MCP operations (store, recall, lists, stats, and duplicate scans) in that directory tree. If no `.recalld.toml` is found, the default namespace is `"default"`.

### Optional by-ID namespace assertions

`get_memory`, `reinforce_memory`, `forget_memory`, and single-mode `find_similar_memories` accept an optional `namespace` string. It asserts the namespace of the canonical record; it does not select a different scope. Omission keeps UUID-based scope even when the session default differs. An explicit mismatch (including a nonexistent namespace) fails with `NAMESPACE_MISMATCH` before embedding, access recording, or mutation. Unknown UUIDs remain not found. Null, empty strings, and invalid names are rejected as assertions.

HTTP uses `?namespace=name` on `GET /memories/:id` and `DELETE /memories/:id`, and a JSON `namespace` field on `POST /memories/:id/reinforce` and `POST /similar/:id`. Mismatches return HTTP 409 with error code `NAMESPACE_MISMATCH`.

Daemon forwarding uses dedicated `*_scoped` RPC methods. Combining a namespace assertion with `recordAccess=false` uses `get_memory_scoped_read_only` or `find_similar_scoped_read_only`; these require a namespace and force nonobserving retrieval. Older daemons reject unknown methods, and the client never retries through a weaker method. Existing assertion-only RPCs reject `recordAccess` fields, and existing read-only RPCs reject namespace assertions. Assertion-only true mode omits the wire flag to remain compatible. These are additive v1 controls, not authentication, receipts, or a transactional whole-store contract. Batch assertions remain outside this slice.

### Connection context

Read the MCP resource `recalld://session` to inspect the connection's frozen default namespace, its canonical name/ID and configured dimension when it exists, current v1 limits, and bounded duplicate-scan semantics. A missing namespace is reported with `exists: false` and `resolved: null`; the read does not create it. `source: "bridge"` identifies the stored connection default, not whether it originally came from a configuration file or a fallback.

Session resource reads carry `cacheScope: "private"` and `ttlMs: 0`; the public resource catalog contains only a static description. The resource advertises only application contract v1. It does not claim active embedding-profile compatibility, generations, receipts, idempotency, exact scans, or future read-only flags. The dimension is namespace configuration, not an active provider identity. HTTP REST `GET /capabilities` remains unimplemented.
