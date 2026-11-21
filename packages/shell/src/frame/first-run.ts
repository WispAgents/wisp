// The first-run launcher (v1 launcher.js firstRun, F-comp-35): until your first task the box is the prompt, how it runs
// (mode, harness) and Send; Where, vs, Delegate, Goal, Worktree and Later wait behind More options. Once opened, or
// once there's a task of yours, it stays full, kept in this browser.
import type { Json, PluginClient, PluginStore } from '@wisp/client';
import type { LauncherTool } from '@wisp/ui';

/** what a first run keeps of each group of the bar: attach (it's how a prompt gets its files) and the phone's mic */
const KEEP: Readonly<Record<'lead' | 'mid' | 'tail', readonly string[]>> = { lead: ['attach', 'mic'], mid: [], tail: [] };

/** The bar as drawn: full, or the first run's. */
export type BarMode = 'full' | 'first';

/** What the bar is now, from what it drew (ADR-0017: nothing jumps while it loads). Full once opened (`full`) or with a
 *  task of yours (`mine`); the first run only once the data says so: the saved answer in (`ready`) and the task list
 *  in whole (`loaded`), with none of yours. Until then the bar keeps what it drew, so it never goes full, first run,
 *  full again as the stores come in (that replaced its tools and lost a tap on More). It starts full: a returning
 *  user's bar, the common case. */
export function barMode(drawn: BarMode, d: { readonly full: boolean; readonly mine: boolean; readonly ready: boolean; readonly loaded: boolean }): BarMode {
  if (d.full || d.mine) return 'full';
  return d.ready && d.loaded ? 'first' : drawn;
}

/** The bar's group as the first run shows it: what it keeps, and More options at the end of the tail. */
export function firstRunTools(first: boolean, group: 'lead' | 'mid' | 'tail', tools: readonly LauncherTool[], more?: LauncherTool): LauncherTool[] {
  if (!first) return [...tools];
  return [...tools.filter((x) => KEEP[group].includes(x.id)), ...(group === 'tail' && more ? [more] : [])];
}

/** keyed by the root client: `plugin('shell')` gives a new client each call, and a store is defined once */
const stores = new WeakMap<object, PluginStore<Json>>();

/** Whether the launcher has been opened full in this browser (the shell's client's store). */
export function launcherFull(root: { plugin(id: string): PluginClient }): PluginStore<Json> {
  const had = stores.get(root);
  if (had) return had;
  const client = root.plugin('shell');
  const store = client.defineStore<Json>({ id: 'shell.launcherFull', version: 1, defaults: false, persist: true, check: (v) => (typeof v === 'boolean' ? v : undefined) });
  stores.set(root, store);
  return store;
}
