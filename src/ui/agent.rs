//! The name this process announces to the daemon (`hello`), for activity
//! attribution. An MCP stdio process serves exactly one client, so the
//! name it learns from `clientInfo` is process-wide.

use std::sync::RwLock;

static AGENT: RwLock<Option<(String, &'static str)>> = RwLock::new(None);

/// Record the client's name; `kind` is `"mcp"` or `"ui"`.
pub fn set(name: &str, kind: &'static str) {
    let name = name.trim();
    if name.is_empty() || name.len() > 128 {
        return;
    }
    let mut slot = AGENT.write().unwrap_or_else(|e| e.into_inner());
    if slot.as_ref().is_none_or(|(n, _)| n != name) {
        *slot = Some((name.to_owned(), kind));
    }
}

/// `(name, kind)` once known.
pub fn get() -> Option<(String, &'static str)> {
    AGENT.read().unwrap_or_else(|e| e.into_inner()).clone()
}
