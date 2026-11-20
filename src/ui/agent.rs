//! The name this process announces to the daemon (`hello`), for activity
//! attribution. An MCP stdio process serves exactly one client, so the
//! name it learns from `clientInfo` is process-wide.

use std::sync::{Mutex, RwLock};

static AGENT: RwLock<Option<(String, &'static str)>> = RwLock::new(None);

type Listener = Box<dyn Fn(&str, &'static str) + Send + Sync>;
static LISTENERS: Mutex<Vec<Listener>> = Mutex::new(Vec::new());

/// Call `f` whenever the name changes (`recalld mcp` names its daemon client).
pub fn on_set(f: impl Fn(&str, &'static str) + Send + Sync + 'static) {
    LISTENERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Box::new(f));
}

/// Record the client's name; `kind` is `"mcp"` or `"ui"`.
pub fn set(name: &str, kind: &'static str) {
    let name = name.trim();
    if name.is_empty() || name.len() > 128 {
        return;
    }
    let mut slot = AGENT.write().unwrap_or_else(|e| e.into_inner());
    if slot.as_ref().is_none_or(|(n, _)| n != name) {
        *slot = Some((name.to_owned(), kind));
        drop(slot);
        for listener in LISTENERS.lock().unwrap_or_else(|e| e.into_inner()).iter() {
            listener(name, kind);
        }
    }
}
