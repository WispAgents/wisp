// Mark done, watched from the plugin's start (index.ts setup): a pending op setting a task's state to done, from a
// command not seen before. What asks about it (ask.ts, its strings, the dialog's words) loads only when one comes
// (ADR-0105); what was in flight at start (restored after a reload) was asked about before, so never again.
import { subscribe, type EntityKey, type PluginClient } from '@wisp/client';
import { afterFirstScreen } from '@wisp/ui/pure';
import { newlyDone } from './done-watch.ts';

export function watchForDone(c: PluginClient): void {
  const seen = new Set(c.entities.pending()().map((p) => p.mid));
  /** Dones whose asker hasn't loaded yet (a failed fetch: tried again with the next change, never dropped) */
  const waiting = new Set<EntityKey>();
  const ask = (): void => {
    if (!waiting.size) return;
    void import('./ask.ts').then(
      (m) => {
        const keys = [...waiting];
        waiting.clear();
        for (const key of keys) void m.askAbout(c, key);
      },
      () => undefined,
    );
  };
  const stop = subscribe(c.entities.pending(), (list) => {
    for (const key of newlyDone(list, seen)) waiting.add(key);
    ask();
  });
  c.scope.add(stop);
  // fetched once the first screen is up, so a Done rarely waits on it
  afterFirstScreen(() => void import('./ask.ts').catch(() => undefined));
}
