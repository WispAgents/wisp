// After Mark done on this client (design/worktrees-on-done.md): the folders the task and what ran under it
// hold. The setting decides (Keep pins them, Remove takes the clean ones), and what's left to decide opens
// the dialog (`worktrees.folders`, an overlay view). The rows are kept here by task, so the dialog opens on
// them without asking again; a dialog restored from the URL asks for them itself.

import { entityKey, signal, subscribe, type EntityKey, type ExtensionId, type PeerId, type PluginClient } from '@wisp/client';
import { doneApply, doneList } from './commands.ts';
import { newlyDone, planDone, SESSION } from './done.ts';
import type { DoneFolder } from './types.ts';

import { VIEW } from './view-id.ts';

export { VIEW };

export interface Asked {
  readonly folders: readonly DoneFolder[];
  readonly only: boolean;
}

/** what each open question is about, by the task's entity key */
export const asked = signal<Readonly<Record<string, Asked>>>({});

export const peerOf = (key: string): PeerId => key.slice(0, key.indexOf(':')) as PeerId;
export const bare = (key: string): string => key.slice(key.indexOf(':') + 1);

const quiet = (p: Promise<unknown>): void => {
  p.catch(() => {}); // a refusal's toast comes from the client's onError
};

/** Resolves once no command of this client still holds the task's Done in flight (or after 10 s). */
function settled(c: PluginClient, key: EntityKey): Promise<void> {
  const holds = (): boolean => c.entities.pending()().some((p) => p.ops.some((o) => o.e === SESSION && o.id === key));
  if (!holds()) return Promise.resolve();
  return new Promise((done) => {
    let stop = (): void => {};
    const cancel = c.scope.timeout(10_000, () => (stop(), done()));
    stop = subscribe(c.entities.pending(), () => {
      if (!holds()) (stop(), cancel(), done());
    });
  });
}

/** One task this client marked done: answer by the setting, or ask. */
export async function askAbout(c: PluginClient, key: EntityKey): Promise<void> {
  await settled(c, key);
  // the Done was refused (its toast says so) or undone already: nothing to ask
  if ((c.entities.peek<{ state?: string }>(SESSION, key) as { state?: string } | undefined)?.state !== 'done') return;
  const peer = peerOf(key);
  const sessionId = bare(key);
  let list;
  try {
    list = await c.run(doneList, { peer, sessionId, sizes: false });
  } catch {
    return; // no list: the hourly cleanup still looks at it
  }
  const plan = planDone(list.onDone, list.folders);
  if (plan.keep.length) quiet(c.run(doneApply, { peer, sessionId, keep: plan.keep, keepTasks: list.folders.map((f) => f.task) }));
  if (plan.remove.length) quiet(c.run(doneApply, { peer, sessionId, remove: plan.remove }));
  if (plan.ask.length === 0) return;
  asked({ ...asked(), [key]: { folders: plan.ask, only: plan.only } });
  c.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: VIEW as ExtensionId, params: { key, only: plan.only, n: plan.ask.length } } });
}

/** What's in flight at start (restored from disk after a reload): asked about before, so never again. */
export const inFlight = (c: PluginClient): Set<string> => new Set(c.entities.pending()().map((p) => p.mid));

/** Watches this client's commands for a Done, for as long as the plugin's UI is on. `seen`: what was in flight when the
 * plugin started (this module loads apart from the first page, ADR-0011: a Done in between is still asked about). */
export function watchDone(c: PluginClient, seen: Set<string> = inFlight(c)): void {
  for (const key of newlyDone(c.entities.pending()(), seen)) void askAbout(c, key);
  const stop = subscribe(c.entities.pending(), (list) => {
    for (const key of newlyDone(list, seen)) void askAbout(c, key);
  });
  c.scope.add(stop);
}

/** The dialog's rows for a task: kept from the question, or asked again (a dialog restored from the URL). */
export async function rowsFor(c: PluginClient, key: string, only: boolean): Promise<Asked> {
  const have = asked()[key];
  if (have) return have;
  const list = await c.run(doneList, { peer: peerOf(key), sessionId: bare(key), sizes: false });
  const plan = planDone(only ? 'remove' : 'ask', list.folders);
  return { folders: plan.ask, only };
}

/** The question is answered (or closed): its rows go. */
export function forget(key: string): void {
  const { [key]: _gone, ...rest } = asked();
  asked(rest);
}

/** The `setting` row of the task's wisp, for Always do this (hidden when a formula or plugin locks it). */
export const settingKey = (key: string, setting: string): EntityKey => entityKey(peerOf(key), setting);
