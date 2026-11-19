// Builds the ListEnv once per mounted list: the shell's alien signals turned into Solid accessors
// (navigation, the keyboard's task, the agents), one clock for the rows' times owned by a child of
// the plugin's scope (so it stops with the list), the action context, and the list's local state.

import { createEffect, createMemo, createSignal, onCleanup } from 'solid-js';
import { useSignal } from '@wisp/ui';
import { ownerOf, waitingToSend, type EntityKey, type PluginClient } from '@wisp/client';
import type { ShellContext } from '@wisp/shell/contract';
import { readOnOpen, type ActionCtx } from '../actions.ts';
import { flatten, taskKeys } from '../model/flat.ts';
import { machineLabelKey } from '../model/machine.ts';
import { modelOf } from '../state.ts';
import { KIND, type Load } from '../types.ts';
import type { ListEnv } from './env.ts';
import '../i18n/lazy.ts';

/** how often the rows' times move on */
export const TICK_MS = 30_000;

export function buildEnv(shell: ShellContext, client: PluginClient): ListEnv {
  const model = modelOf(client);
  // the project the top bar's chips picked: the list shows it (one filter, v1 wisp.filter)
  const picked = useSignal(shell.projectFilter?.value ?? (() => undefined));
  createEffect(() => {
    const f = picked();
    if (f !== undefined && f !== model.view.value().filter) model.setFilter(f);
  });
  // the phone apps' computer filter (the Tasks bar's title): 'all', or one gone from the phone, is every computer
  const computer = useSignal(shell.computerFilter?.value ?? (() => 'all'));
  const peers = useSignal(client.peers.list());
  createEffect(() => {
    const c = computer();
    model.setComputer(c && c !== 'all' && peers().includes(c as never) ? c : undefined);
  });
  const nav = useSignal(shell.nav);
  const selected = useSignal(shell.selection.get);
  const agents = useSignal(model.agents);
  const load = useSignal(client.entities.row<Load>(KIND.load, 'here' as EntityKey));
  const scope = client.scope.child('list');
  onCleanup(() => scope.dispose());
  const [now, setNow] = createSignal(scope.timers.now());
  scope.interval(TICK_MS, () => setNow(scope.timers.now()));
  const [menuFor, setMenuFor] = createSignal<EntityKey | undefined>(undefined);
  const [deleting, askDelete] = createSignal<EntityKey | undefined>(undefined);
  const open = createMemo(() => {
    const m = nav().main;
    return m.kind === 'task' ? (m.id as unknown as EntityKey) : undefined;
  });
  const split = (): boolean => nav().layout === 'split';
  const swipes = shell.platform.phone;
  // the rows a command waits to send for: one set for the list, it changes only when a command or a computer does
  const waiting = useSignal(() => waitingToSend(client.entities.pending()(), (peer) => client.peers.status(peer)().kind === 'online'));
  const here = (): string => shell.t(machineLabelKey(load()?.platform));
  // a harness by the name its plugin registered ("Codex"), on every line of a row, as the run line names it (audit D #2)
  const harnesses = useSignal(shell.harnesses());
  const named = (id: string): string | undefined => harnesses().find((h) => h.id === id)?.name;
  /** the keyboard moves to the row's neighbour when the row it is on leaves the list */
  const handOff = (key: EntityKey): void => {
    if (selected() !== key) return;
    const order = taskKeys(flatten(model.sections(), { toggled: new Set(model.view.value().toggled), open: open(), split: split() }));
    const i = order.indexOf(key);
    shell.selection.set(order[i + 1] ?? order[i - 1]);
  };
  const actions: ActionCtx = { client, t: shell.t, toast: shell.toast, now: () => scope.timers.now(), handOff };
  return {
    shell,
    client,
    model,
    actions,
    now,
    open,
    selected,
    split,
    swipes,
    keys: !shell.platform.mobile,
    agents,
    named,
    rowCtx: (key) => ({ split: split(), open: open() === key, swipes, here: here(), sandbox: shell.t('shell-list.run.sandbox'), agents: agents(), named, waiting: waiting().has(key), defaultModel: (s) => shell.defaultModel?.(s.agent, { peer: ownerOf(key), cwd: s.cwd }) }),
    openTask: (key) => {
      shell.selection.set(key);
      client.navigate({ t: 'openTask', id: key as never });
      readOnOpen(actions, key);
    },
    menuFor,
    // a row's menu also makes it the keyboard's row (v1 feed.js contextmenu: setFocus, no scroll), so it shows as
    // picked, its actions in view, while the menu is up
    setMenuFor: (key) => {
      shell.selection.set(key);
      setMenuFor(key);
    },
    menuAt: (key, el) => {
      setMenuFor(key);
      // the one row menu is a context menu around the list: open it under the button
      const r = el.getBoundingClientRect();
      el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: r.left, clientY: r.bottom }));
    },
    deleting,
    // the shell's one Rename task dialog (QA round 15 #2: the head and the row had two)
    askRename: (key) => key && client.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.rename' as never, params: { id: key } } }),
    askDelete: (key) => askDelete(() => key),
  };
}
