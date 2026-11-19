// The global namespace scope (DESIGN §2): one setting, shown in the sidebar
// and the first chip of every toolbar. Persisted across sessions.
const KEY = 'recalld.scope';

function load(): string | null {
  try {
    return localStorage.getItem(KEY) || null;
  } catch {
    return null;
  }
}

class Scope {
  /** `null` means All namespaces. */
  current = $state<string | null>(load());

  set(namespace: string | null) {
    this.current = namespace;
    try {
      if (namespace) localStorage.setItem(KEY, namespace);
      else localStorage.removeItem(KEY);
    } catch {
      /* storage unavailable: keep the in-memory choice */
    }
  }

  get label() {
    return this.current ?? 'All namespaces';
  }
}

export const scope = new Scope();
