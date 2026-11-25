// Settings → Agents' data, for the list and every detail section: the harnesses as they settle (ADR-0066), their
// accounts, sign-ins, keys and tools, which plugins are off, each one's plan windows, and the writes the rows make
// (Update, Update all, Re-check). Made once per pane open, in the pane's owner, and handed to each section.
import { createMemo, createResource, createSignal, onCleanup, type Accessor } from 'solid-js';
import { entityKey, type PluginClient } from '@wisp/client';
import type { HarnessUiDef, useShell } from '@wisp/shell';
import { useSignal } from '@wisp/ui/pure';
import { useDetected } from '../detected.ts';
import { planWindows, pluginsList, recheck, upgrade, upgradeQuiet } from '../commands.ts';
import { fullest, statusOf, withDisabled, type PlanWindow, type PluginState, type Status } from '../harness-pane.ts';
import { t } from '../i18n.ts';
import { ACCOUNT, DETECT_TOPIC, HARNESS, KEY, LOGIN, PLACE, TOOL, TOPIC, loginId, type AccountRow, type HarnessRow, type KeyRow, type LoginRow, type PlaceRow, type ToolRow } from '../types.ts';
import { harnessState, mergeHarnesses, present, recheckSummary, updateAllSummary, withPending } from '../words.ts';
import '../i18n/lazy.ts';

export const quiet = (p: Promise<unknown>): void => {
  p.catch(() => {}); // the shell's onError shows the rollback toast
};

type Shell = ReturnType<typeof useShell>;

function useRows<T>(c: PluginClient, kind: string): Accessor<readonly T[]> {
  const list = useSignal(c.entities.list<T>(kind));
  return () => list() as readonly T[];
}

// size-ok: the pane's one data owner; each part is a line or two, read by every section
export function agentsData(c: PluginClient, shell: Shell) {
  const peer = c.home;
  const scope = c.scope.child('agents-pane');
  onCleanup(() => scope.dispose());
  c.watch(peer, TOPIC, scope);
  // each harness as its detect settles (ADR-0066): the rows show at once, `Checking…` until each answers
  c.watch(peer, DETECT_TOPIC, scope);
  // each harness's mark, as its own plugin registered it (HarnessUiDef.logo: plugins/claude/ui, …); a harness's UI
  // stays registered while its plugin is off, so it's listed, Disabled
  const marks = useSignal(shell.harnesses()) as Accessor<readonly HarnessUiDef[]>;
  const mine = useRows<HarnessRow>(c, HARNESS);
  // this wisp's own: the runner picker may also watch another computer's
  const detected = useDetected(c.entities, peer);
  // which plugins are off, read when the pane opens and after a switch
  const [plugins, { refetch: rereadPlugins }] = createResource(() => c.run(pluginsList, { peer }).catch((): readonly PluginState[] => []));
  // no answer yet (a restart, a first start): every known harness at once, Checking… until its row lands
  const harnesses = createMemo(() => withDisabled(withPending(mergeHarnesses(mine(), detected()), marks(), mine().length === 0), marks(), plugins.latest ?? []));
  const accounts = useRows<AccountRow>(c, ACCOUNT);
  const logins = useRows<LoginRow>(c, LOGIN);
  const keys = useRows<KeyRow>(c, KEY);
  const tools = useRows<ToolRow>(c, TOOL);
  const place = useSignal(c.entities.row<PlaceRow>(PLACE, entityKey(peer, 'this')));
  const logoOf = (id: string): string | undefined => marks().find((m) => m.id === id)?.logo;
  const markOf = (id: string): HarnessUiDef | undefined => marks().find((m) => m.id === id);

  const login = (agent: string, account = ''): LoginRow | undefined => logins().find((l) => l.agent === agent && loginId(l.agent, l.account) === loginId(agent, account));
  const accs = (agent: string): AccountRow[] =>
    accounts()
      .filter((a) => a.agent === agent)
      .sort((a, b) => a.ord - b.ord);
  const keysFor = (agent: string): KeyRow[] => keys().filter((k) => k.agent === agent);
  // its own Sign in (one account): the state says what that login says, never Ready beside Sign in
  const ownLogin = (h: HarnessRow): boolean => !h.disabled && h.signIn === 'cli' && present(h) && (!h.brand || accs(h.id).length <= 1);
  const cli = (h: HarnessRow): boolean | undefined => (ownLogin(h) ? (login(h.id, '')?.signedIn ?? undefined) : undefined);
  const status = (h: HarnessRow): Status => statusOf(h, cli(h));
  const state = (h: HarnessRow): ReturnType<typeof harnessState> => harnessState(h, cli(h));

  // each present harness's fullest plan window (the list's ring), read again as the plan's rows change
  const planRows = useSignal(c.entities.list('budget_acc'));
  const [windows] = createResource(
    () => ({ n: planRows().length, ids: harnesses().filter((h) => present(h) && !h.disabled).map((h) => h.id) }),
    async ({ ids }) => new Map(await Promise.all(ids.map(async (id) => [id, fullest(await c.run(planWindows, { peer, agent: id }).catch(() => []))] as const))),
  );
  const ringOf = (id: string): PlanWindow | undefined => windows.latest?.get(id);

  // TRY-17: harnesses with a newer version, and the ones an Update press is running for (their button stays, pending)
  const [updating, setUpdating] = createSignal<ReadonlySet<string>>(new Set());
  const outdated = createMemo(() => harnesses().filter((h) => present(h) && h.latest && !h.installing && !updating().has(h.id)));
  const done = (ids: readonly string[]): void => {
    setUpdating((s) => {
      const n = new Set(s);
      for (const id of ids) n.delete(id);
      return n;
    });
  };
  const update = (list: readonly HarnessRow[]): void => {
    for (const h of list) {
      setUpdating((s) => new Set(s).add(h.id));
      c.run(upgrade, { peer, agent: h.id })
        .then(() => shell.toast.show(t('accounts.updated', { name: h.name, version: h.latest ?? '' })))
        .catch(() => {}) // the shell's onError shows the rollback toast (with the command to run, for a copy wisp can't update)
        .finally(() => done([h.id]));
    }
  };
  // Update all: every update at once, then one word on each, what took and what didn't with the command to run (audit I
  // IS11: it said nothing when they didn't take)
  const updateAll = (list: readonly HarnessRow[]): void => {
    for (const h of list) setUpdating((s) => new Set(s).add(h.id));
    void Promise.allSettled(list.map((h) => c.run(upgradeQuiet, { peer, agent: h.id }))).then((all) => {
      done(list.map((h) => h.id));
      const said = updateAllSummary(
        all.map((d, i) => {
          const h = list[i];
          const name = h?.name ?? '';
          return d.status === 'fulfilled' ? { name, version: h?.latest ?? '' } : { name, error: d.reason instanceof Error ? d.reason.message : String((d.reason as { message?: unknown } | undefined)?.message ?? d.reason) };
        }),
      );
      shell.toast.show(said.message, { tone: said.failed ? 'error' : 'success', description: said.description });
    });
  };
  const [rechecking, setRechecking] = createSignal(false);
  const recheckAll = (): void => {
    setRechecking(true);
    // v1 recheckSummary: what this Re-check turned up, here and on every machine (the server asks them too)
    const before = new Map(harnesses().map((h) => [h.id, h]));
    c.run(recheck, { peer })
      .then((after) => shell.toast.show(recheckSummary(before, after ?? [])))
      .catch(() => {})
      .finally(() => setRechecking(false));
  };
  // v1 copySetupCommand: on the clipboard, and what to do next
  const copy = (cmd: string): void => {
    void navigator.clipboard?.writeText(cmd).then(
      () => shell.toast.show(t('accounts.setup.copied')),
      () => {},
    );
  };
  return { c, peer, shell, harnesses, marks, logoOf, markOf, plugins: () => plugins.latest ?? [], rereadPlugins, accounts, login, accs, keysFor, tools, place, ownLogin, status, state, ringOf, updating, outdated, update, updateAll, rechecking, recheckAll, copy };
}

export type AgentsData = ReturnType<typeof agentsData>;
