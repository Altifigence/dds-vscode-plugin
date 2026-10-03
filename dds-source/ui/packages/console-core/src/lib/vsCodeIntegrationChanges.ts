const CHANGE_EVENT = 'dds:vscode-integration-changed';
const CHANGE_STORAGE_KEY = 'dds.vscode-integration-change.v1';

/** Share an invalidation between logical windows and same-origin renderers.
 * Installation state always comes from the host, never from this notification. */
export function createVsCodeIntegrationChanges(scope: string | null) {
  return {
    publish() {
      if (!scope || typeof window === 'undefined') return;
      window.dispatchEvent(new CustomEvent(CHANGE_EVENT, { detail: scope }));
      try {
        // A transient storage event reaches other native windows/browser tabs.
        // Remove it immediately so it cannot become a second settings store.
        window.localStorage.setItem(CHANGE_STORAGE_KEY, JSON.stringify({ scope, change: Math.random() }));
        window.localStorage.removeItem(CHANGE_STORAGE_KEY);
      } catch { /* Focus refresh still works when browser storage is unavailable. */ }
    },
    subscribe(onChange: () => void) {
      if (!scope || typeof window === 'undefined') return () => undefined;
      const changed = (event: Event) => {
        if ((event as CustomEvent<unknown>).detail === scope) onChange();
      };
      const stored = (event: StorageEvent) => {
        if (event.key !== CHANGE_STORAGE_KEY || !event.newValue) return;
        try {
          if ((JSON.parse(event.newValue) as { scope?: unknown })?.scope === scope) onChange();
        } catch { /* Ignore malformed notifications; they grant no authority. */ }
      };
      window.addEventListener(CHANGE_EVENT, changed);
      window.addEventListener('storage', stored);
      return () => {
        window.removeEventListener(CHANGE_EVENT, changed);
        window.removeEventListener('storage', stored);
      };
    },
  };
}
