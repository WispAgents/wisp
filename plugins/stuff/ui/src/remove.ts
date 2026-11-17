// Remove with Undo (v1 stuff.js removeItem): the thing leaves the page at once with "Removed “x” from My Stuff" and
// Undo; it's removed for real once the toast has gone. The timer is the plugin's (its scope), not the page's: Undo
// still works after leaving My Stuff, and nothing stays hidden and kept.
import './i18n/lazy.ts';
import type { PluginClient } from '@wisp/client';
import type { ShellContext } from '@wisp/shell';
import type { Entry } from './items.ts';
import { stuffState } from './state.ts';

export const UNDO_MS = 5000;

const waiting = new WeakMap<PluginClient, Map<string, () => void>>();

export function removeWithUndo(c: PluginClient, shell: Pick<ShellContext, 'toast'>, e: Entry, removeNow: () => void): void {
  let pending = waiting.get(c);
  if (!pending) waiting.set(c, (pending = new Map()));
  if (e.it.pending || pending.has(e.key)) return;
  const st = stuffState(c);
  const unhide = (): void => {
    pending.delete(e.key);
    st.setHidden((h) => new Set([...h].filter((k) => k !== e.key)));
  };
  st.setHidden((h) => new Set([...h, e.key]));
  const cancel = c.scope.timeout(UNDO_MS, () => {
    removeNow();
    unhide();
  });
  pending.set(e.key, cancel);
  shell.toast.show(c.t('stuff.toast.removed', { title: e.it.title || e.it.name }), {
    duration: UNDO_MS,
    action: {
      label: c.t('stuff.toast.undo'),
      onClick: () => {
        pending.get(e.key)?.();
        unhide();
      },
    },
  });
}
