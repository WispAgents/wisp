// size-ok: one pane whose list, General and harness detail share the same rows, sign-in controls and dialogs
// Settings → Agents (v1 settings.js paintAgents, agent-login.js; design/v1-reference/settings.agents), as a list and a
// detail (harness audit r2): a search over General and each harness (its name and the settings it holds), General's
// GitHub and git, Lean runs, Ask before a changed harness, updates and Re-check; and each harness's detail: its state,
// version and sign-in, Claude's and Codex's extra accounts ("Add another … account"), a CLI sign-in's steps, a key for
// harnesses that take one (opencode), Install and Update, the notice when its program changed, then its program and
// version, its switch (its plugin), its default model, thinking level and mode for a new task (`runtime.launch`, what
// New task reads), and what other plugins add (wisp.budget's stop line). Every write shows at once.
import { createMemo, createResource, createSignal, For, Match, onCleanup, Show, Switch as Pick, type JSX } from 'solid-js';
import { entityKey, type PluginClient } from '@wisp/client';
import {
  BROWSER_CAN_UNKNOWN,
  BROWSER_SETTING,
  BROWSERS_SETTING,
  BROWSER_VALUES,
  LAUNCH_SETTING,
  MODES_FOR_HARNESS,
  Slot,
  browserGlobal,
  browserOwn,
  browserRow,
  defaultModels,
  harnessDefaults,
  useShell,
  withBrowser,
  withHarnessDefaults,
  type BrowserCan,
  type BrowserValue,
  type HarnessDefaults,
} from '@wisp/shell';
import {
  Button,
  CopyCommand,
  Dialog,
  Inline,
  Input,
  Link,
  Menu,
  Pill,
  SelectSetting,
  SettingsEmpty,
  SettingsGroup,
  SettingsInset,
  SettingsInsetAdd,
  SettingsInsetRow,
  SettingsMasterDetail,
  SettingsRow,
  Stack,
  Switch,
  Text,
  useSignal,
  Wrap,
} from '@wisp/ui';
import { useDetected } from '../detected.ts';
import { fromRow, text } from '../source.ts';
import { Steps } from './Steps.tsx';
import { Docs, HowTo, Keys } from './AgentParts.tsx';
import { addAccount, install, keyRemove, keySet, loginCancel, loginInput, loginStart, logout, pluginList, pluginSet, planView, recheck, removeAccount, setAsk, settingSet, trust, updateAccount, upgrade, upgradeQuiet } from '../commands.ts';
import { GENERAL, modelProblem, navItems, shownItem, type NavRows } from '../agents-nav.ts';
import { AddAccount, RemoveAccount, RenameAccount } from './AccountDialogs.tsx';
import { t } from '../i18n.ts';
import { ACCOUNT, DETECT_TOPIC, HARNESS, KEY, LOGIN, PLACE, TOOL, TOPIC, loginId, type AccountKind, type AccountRow, type DetectRow, type HarnessRow, type KeyRow, type LoginRow, type PlaceRow, type ToolRow } from '../types.ts';
import { accOf, accountMenu, accountState, withPending, type Tone, accountLine, changeTip, changeTitle, changeWhat, harnessLine, harnessState, howToGet, loginControl, masked, mergeHarnesses, present, recheckSummary, site, sentence, stillChanged, toolState, updateAllSummary } from '../words.ts';
import '../i18n/lazy.ts';

const quiet = (p: Promise<unknown>): void => {
  p.catch(() => {}); // the shell's onError shows the rollback toast
};

type Rows<T> = () => readonly T[];

function useRows<T>(c: PluginClient, kind: string): Rows<T> {
  const list = useSignal(c.entities.list<T>(kind));
  return () => list() as readonly T[];
}

/** The settings a harness's detail holds, by title key: the list's search finds a harness by them. */
export const DETAIL_ROWS = [
  'accounts.detail.signIn',
  'accounts.detail.program',
  'accounts.detail.enabled',
  'accounts.detail.model',
  'accounts.detail.effort',
  'accounts.detail.mode',
  'accounts.detail.browser',
  'accounts.detail.budget',
] as const;

export default function Agents(props: { readonly client: PluginClient; readonly filter?: string }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  // each harness's mark, as its own plugin registered it (HarnessUiDef.logo: plugins/claude/ui, …)
  const marks = useSignal(shell.harnesses());
  // v1 copySetupCommand: on the clipboard, and what to do next
  const copy = (cmd: string): void => {
    void navigator.clipboard?.writeText(cmd).then(
      () => shell.toast.show(t('accounts.setup.copied')),
      () => {},
    );
  };
  const logoOf = (id: string): string | undefined => marks().find((m) => m.id === id)?.logo;
  const peer = c.home;
  const scope = c.scope.child('agents-pane');
  onCleanup(() => scope.dispose());
  c.watch(peer, TOPIC, scope);
  // each harness as its detect settles (ADR-0066): the rows show at once, `Checking…` until each answers
  c.watch(peer, DETECT_TOPIC, scope);
  const mine = useRows<HarnessRow>(c, HARNESS);
  // this wisp's own: the runner picker may also watch another computer's
  const detected = useDetected(c.entities, peer);
  // no answer yet (a restart, a first start): every known harness at once, Checking… until its row lands
  const harnesses = createMemo(() => withPending(mergeHarnesses(mine(), detected()), marks(), mine().length === 0));
  const accounts = useRows<AccountRow>(c, ACCOUNT);
  const logins = useRows<LoginRow>(c, LOGIN);
  const keys = useRows<KeyRow>(c, KEY);
  const tools = useRows<ToolRow>(c, TOOL);
  const place = useSignal(c.entities.row<PlaceRow>(PLACE, entityKey(peer, 'this')));
  const [keysOf, setKeysOf] = createSignal<HarnessRow | undefined>();
  const [rechecking, setRechecking] = createSignal(false);
  // TRY-17: harnesses with a newer version, and the ones an Update press is running for (their button stays, pending)
  const [updating, setUpdating] = createSignal<ReadonlySet<string>>(new Set());
  const outdated = createMemo(() => harnesses().filter((h) => present(h) && h.latest && !h.installing && !updating().has(h.id)));
  const update = (list: readonly HarnessRow[]): void => {
    for (const h of list) {
      setUpdating((s) => new Set(s).add(h.id));
      c.run(upgrade, { peer, agent: h.id })
        .then(() => shell.toast.show(t('accounts.updated', { name: h.name, version: h.latest ?? '' })))
        .catch(() => {}) // the shell's onError shows the rollback toast (with the command to run, for a copy wisp can't update)
        .finally(() =>
          setUpdating((s) => {
            const n = new Set(s);
            n.delete(h.id);
            return n;
          }),
        );
    }
  };
  // Update all: every update at once, then one word on each, what took and what didn't with the command to run (audit I
  // IS11: it said nothing when they didn't take)
  const updateAll = (list: readonly HarnessRow[]): void => {
    for (const h of list) setUpdating((s) => new Set(s).add(h.id));
    void Promise.allSettled(list.map((h) => c.run(upgradeQuiet, { peer, agent: h.id }))).then((done) => {
      setUpdating((s) => {
        const n = new Set(s);
        for (const h of list) n.delete(h.id);
        return n;
      });
      const said = updateAllSummary(
        done.map((d, i) => {
          const h = list[i];
          const name = h?.name ?? '';
          return d.status === 'fulfilled' ? { name, version: h?.latest ?? '' } : { name, error: d.reason instanceof Error ? d.reason.message : String((d.reason as { message?: unknown } | undefined)?.message ?? d.reason) };
        }),
      );
      shell.toast.show(said.message, { tone: said.failed ? 'error' : 'success', description: said.description });
    });
  };
  // Re-check: General's row and each harness's Program row (v1 recheckSummary: what it turned up, here and on every
  // machine, the server asks them too)
  const recheckNow = (): void => {
    setRechecking(true);
    const before = new Map(harnesses().map((h) => [h.id, h]));
    c.run(recheck, { peer })
      .then((after) => shell.toast.show(recheckSummary(before, after ?? [])))
      .catch(() => {})
      .finally(() => setRechecking(false));
  };
  // keyed by id: a harness that settles updates its row in place (no remount, nothing moves)
  const byId = createMemo(() => new Map(harnesses().map((h) => [h.id, h])));
  const ids = createMemo(() => harnesses().map((h) => h.id), undefined, { equals: (a, b) => a.length === b.length && a.every((x, i) => x === b[i]) });
  const login = (agent: string, account = ''): LoginRow | undefined => logins().find((l) => l.agent === agent && loginId(l.agent, l.account) === loginId(agent, account));
  const accs = (agent: string): AccountRow[] =>
    accounts()
      .filter((a) => a.agent === agent)
      .sort((a, b) => a.ord - b.ord);
  const keysFor = (agent: string): KeyRow[] => keys().filter((k) => k.agent === agent);

  /** `row`: the harness's (or GitHub's) row as it is now: its id, and whether it's known signed out (it updates in place
   *  as its detect settles) */
  const signControl = (row: () => { readonly id: string; readonly signedIn?: boolean | null | undefined }, account: string, isAccount: boolean): JSX.Element => {
    // no sign-in yet: what the row knows (GitHub's row says gh is signed in before any sign-in from here)
    const known = (): LoginRow | undefined => login(row().id, account) ?? (row().signedIn === true ? { agent: row().id, account, state: 'idle', signedIn: true } : undefined);
    const ctl = (): ReturnType<typeof loginControl> => loginControl(known(), { account: isAccount, out: row().signedIn === false });
    const args = { peer, agent: row().id, account };
    return (
      <Pick>
        <Match when={ctl() === 'cancel'}>
          <Button size="sm" variant="ghost" onClick={() => quiet(c.run(loginCancel, args))}>
            {t('accounts.login.cancel')}
          </Button>
        </Match>
        <Match when={ctl() === 'signIn'}>
          <Button size="sm" onClick={() => quiet(c.run(loginStart, args))}>
            {t('accounts.login.signIn')}
          </Button>
        </Match>
        <Match when={ctl() === 'signedIn'}>
          <Inline gap="3">
            <Pill tone="good" icon="check" variant="plain">
              {t('accounts.login.signedIn')}
            </Pill>
            <Button size="xs" variant="ghost" onClick={() => quiet(c.run(logout, args))}>
              {t('accounts.login.signOut')}
            </Button>
          </Inline>
        </Match>
      </Pick>
    );
  };

  // v1 addBox: a name (blank: "Claude 2"), Personal or Work and the notice, then its row and its sign-in at once
  const [adding, setAdding] = createSignal<HarnessRow | undefined>();
  const addAnother = (h: HarnessRow, label: string, kind: AccountKind): void => {
    const n = accs(h.id).length + 1;
    quiet(c.run(addAccount, { peer, agent: h.id, id: `a${Date.now().toString(36)}`, label: label || t('accounts.account.n', { brand: h.brand ?? h.name, n }), kind }));
  };
  // the account whose Rename or Remove box is open
  const [renaming, setRenaming] = createSignal<AccountRow | undefined>();
  const [removing, setRemoving] = createSignal<AccountRow | undefined>();
  /** v1 accounts.js moreMenu, every action on its account (the row's account is after its harness in its id) */
  const onAccount = (a: AccountRow, v: string): void => {
    const args = { peer, agent: a.agent, account: accOf(a) };
    if (v === 'rename') setRenaming(a);
    if (v === 'kind') quiet(c.run(updateAccount, { ...args, kind: a.kind === 'work' ? 'personal' : 'work' }));
    if (v === 'up' || v === 'down') quiet(c.run(updateAccount, { ...args, move: v }));
    if (v === 'out') quiet(c.run(logout, args));
    if (v === 'remove') setRemoving(a);
  };

  // ── the list and the detail (harness audit r2): General, then one item per harness ──
  const [query, setQuery] = createSignal('');
  // the pick survives a re-render and a search that hides it; a link from Usage (or the settings search) opens on it
  const [picked, setPicked] = createSignal<string>(props.filter && props.filter !== '' ? props.filter : GENERAL);
  // each harness's plan, for its dot at its limit (quiet: without wisp.budget, no dot)
  const [limits] = createResource(ids, async (list) => {
    const full = await Promise.all(
      list.map((id) =>
        c.run(planView, { peer, agent: id }).then(
          (accs) => [id, accs.some((a) => a.windows.some((w) => w.used >= 1))] as const,
          () => [id, false] as const,
        ),
      ),
    );
    return new Map(full);
  });
  const navRows = (): NavRows => ({
    general: {
      label: t('accounts.general'),
      rows: [t('accounts.tool.github'), 'git', t('accounts.lean.title'), t('accounts.browser.title'), t('accounts.ask.title'), t('accounts.updateAll.title'), t('accounts.recheck.title')],
    },
    harness: DETAIL_ROWS.map((k) => t(k)),
    atLimit: t('accounts.state.atLimit'),
  });
  const stateOf = (h: HarnessRow): { tone: Tone; label: string } => {
    const own = h.signIn === 'cli' && present(h) && (!h.brand || accs(h.id).length <= 1);
    return harnessState(h, own ? (login(h.id, '')?.signedIn ?? undefined) : undefined);
  };
  const items = createMemo(() =>
    navItems(
      harnesses().map((h) => ({ id: h.id, name: h.name, state: stateOf(h), atLimit: limits.latest?.get(h.id) === true, logo: logoOf(h.id) })),
      query(),
      navRows(),
    ),
  );
  const shown = (): string | undefined => shownItem(items(), picked());
  // what a harness's detail reads: its detect row (models, levels, program), its plugin's switch, its defaults
  const detectOf = (id: string): DetectRow | undefined => detected().find((r) => r.id === id);
  const [plugins, { refetch: plugins_ }] = createResource(() => c.run(pluginList, { peer }).catch(() => []));
  const launch = useSignal(c.entities.row<{ value?: unknown }>('setting', entityKey(peer, LAUNCH_SETTING)));
  const defaultsOf = (id: string): HarnessDefaults => harnessDefaults(launch()?.value, id);
  const setDefaults = (id: string, change: Parameters<typeof withHarnessDefaults>[2]): void =>
    quiet(c.run(settingSet, { peer, key: LAUNCH_SETTING, value: withHarnessDefaults(launch()?.value, id, change) }));
  const defaults = defaultModels(shell.client);
  // the browser (ADR-0128): General's is every harness's, a harness's detail sets its own over it
  const browserAll = fromRow(c, BROWSER_SETTING, 'default');
  const browserEach = fromRow(c, BROWSERS_SETTING, {});

  /** One harness's detail: what its row had (state, sign-in, keys, install and update, a changed program, its accounts),
   *  then its program, its switch, its defaults for a new task, and what other plugins add (wisp.budget's stop line). */
  const HarnessDetail = (p: { readonly id: string }): JSX.Element => {
    const id = p.id;
    // the last row it had: a harness that goes (its plugin switched off) may be read once more before its row unmounts
    let last = byId().get(id) as HarnessRow;
    const h = (): HarnessRow => (last = byId().get(id) ?? last);
    // its own Sign in (one account): the state says what that login says, never Ready beside Sign in
    const ownLogin = (): boolean => h().signIn === 'cli' && present(h()) && (!h().brand || accs(h().id).length <= 1);
    const state = () => harnessState(h(), ownLogin() ? (login(h().id, '')?.signedIn ?? undefined) : undefined);
    const det = (): DetectRow | undefined => detectOf(id);
    const plugin = (): { id: string; enabled: boolean } | undefined => (plugins.latest ?? []).find((x) => x.id === `wisp.${id}`);
    const failedKey = (): KeyRow | undefined => keysFor(id).find((k) => k.test && !k.test.ok);
    const models = (): { id: string; label: string }[] => (det()?.agent.models ?? h().models).filter((m) => m.id !== '');
    const efforts = (): string[] => det()?.agent.efforts ?? [];
    const runs = (): string | undefined => defaults.topic(peer, id);
    const bad = (s: string): JSX.Element => (
      <Text size="sm" tone="bad" part="accounts.detail.error">
        {s}
      </Text>
    );
    return (
      <>
        <SettingsGroup part="accounts.harnesses">
          <SettingsRow
            part="accounts.harness"
            title={h().name}
            {...(logoOf(h().id) && { logo: logoOf(h().id) as string })}
            description={
              <Show when={howToGet(h())} fallback={harnessLine(h(), accs(h().id).length)}>
                {(how) => <HowTo tip={how().tip} command={how().command} onCopy={copy} />}
              </Show>
            }
            control={
              <Wrap gap="3" align="center">
                <Show when={h().signIn === 'key' && present(h())}>
                  <Button size="sm" variant="primary" icon="lock" part="accounts.keys.open" onClick={() => setKeysOf(h())}>
                    {keysFor(h().id).some((k) => k.set) ? t('accounts.keys.edit') : t('accounts.keys.add')}
                  </Button>
                </Show>
                <Show when={ownLogin()}>{signControl(h, '', false)}</Show>
                <Show when={!h().checking && !present(h()) && !h().installable}>
                  <Docs name={h().name} url={h().setup?.url} />
                </Show>
                <Show when={!h().checking && !present(h()) && h().installable === true}>
                  <Button size="sm" disabled={h().installing === true} onClick={() => quiet(c.run(install, { peer, agent: h().id }))}>
                    {h().installing === true ? t('accounts.installing') : t('accounts.install')}
                  </Button>
                </Show>
                <Show when={updating().has(h().id) || (!h().checking && present(h()) && h().latest)}>
                  <Button size="sm" part="accounts.update" disabled={updating().has(h().id) || h().installing === true} onClick={() => update([h()])}>
                    {updating().has(h().id) || h().installing === true ? t('accounts.updating') : t('accounts.update', { version: h().latest ?? '' })}
                  </Button>
                </Show>
                <Pill tone={state().tone} dot variant="plain">
                  {state().label}
                </Pill>
              </Wrap>
            }
          />
          <Show when={stillChanged(h()) && h().changed}>
            {(ch) => (
              <SettingsRow
                part="accounts.changed"
                title={changeTitle(h().name, ch())}
                description={<span title={changeTip(ch())}>{t('accounts.change.whatWhy', { what: sentence(changeWhat(ch())) })}</span>}
                control={
                  <Button size="sm" onClick={() => quiet(c.run(trust, { peer, agent: h().id }))}>
                    {t('accounts.change.trust')}
                  </Button>
                }
              />
            )}
          </Show>
          {/* the harness's own sign-in, unless its accounts are listed: then it's under its account's row */}
          <Show when={!(h().brand && present(h()) && accs(h().id).length > 1)}>
            <Steps c={c} agent={h().id} account="" login={login(h().id)} />
          </Show>
          <Show when={h().brand && present(h())}>
            <SettingsInset part="settings.accounts">
              <For each={accs(h().id).length > 1 ? accs(h().id) : []}>
                {(a, i) => (
                  <>
                    <SettingsInsetRow
                      part="accounts.account"
                      title={a.label}
                      description={accountLine(a)}
                      state={accountState(a)}
                      control={
                        <Inline gap="2">
                          {signControl(() => ({ id: h().id, signedIn: a.state === 'signedOut' ? false : null }), accOf(a), true)}
                          <Menu
                            part="accounts.account.menu"
                            trigger={{ icon: 'dots', 'aria-label': t('accounts.account.more', { name: a.label }), variant: 'ghost', size: 'sm' }}
                            items={accountMenu(a, i(), accs(h().id).length)}
                            onSelect={(v) => onAccount(a, v)}
                          />
                        </Inline>
                      }
                    />
                    <Steps c={c} agent={h().id} account={accOf(a)} login={login(h().id, accOf(a))} inset />
                  </>
                )}
              </For>
              <SettingsInsetAdd part="accounts.add" label={t('accounts.account.add', { brand: h().brand ?? h().name })} onClick={() => setAdding(h())} />
            </SettingsInset>
          </Show>
          {/* a key that failed its last Test says so here, not only in the keys box */}
          <Show when={failedKey()}>
            {(k) => (
              <SettingsRow
                part="accounts.detail.key"
                title={t('accounts.detail.keys')}
                description={bad(t('accounts.detail.keyFailed', { name: k().name, message: k().test?.message ?? '' }))}
                control={
                  <Button size="sm" onClick={() => setKeysOf(h())}>
                    {t('accounts.keys.edit')}
                  </Button>
                }
              />
            )}
          </Show>
        </SettingsGroup>
        <SettingsGroup part="accounts.detail" title={t('accounts.detail.title')}>
          <SettingsRow
            part="accounts.detail.program"
            title={t('accounts.detail.program')}
            description={
              <Show when={present(h()) || h().checking} fallback={bad(t('accounts.detail.missing', { name: h().name }))}>
                <Stack gap="1">
                  <Text size="sm" mono>
                    {det()?.agent.bin ?? t('accounts.detail.noPath')}
                  </Text>
                  <Text size="sm" tone="muted">
                    {h().version ? t('accounts.detail.version', { version: h().version ?? '' }) : t('accounts.line.checking')}
                  </Text>
                </Stack>
              </Show>
            }
            control={
              <Button size="sm" variant="ghost" icon="replay" part="accounts.detail.recheck" disabled={rechecking()} onClick={recheckNow}>
                {rechecking() ? t('accounts.recheck.running') : t('accounts.recheck')}
              </Button>
            }
          />
          <Show when={plugin()}>
            {(pl) => (
              <SettingsRow
                part="accounts.detail.enabled"
                title={t('accounts.detail.enabled')}
                description={t('accounts.detail.enabledHint', { name: h().name })}
                control={
                  <Switch
                    aria-label={t('accounts.detail.enabled')}
                    checked={pl().enabled}
                    onChange={(on) => quiet(c.run(pluginSet, { peer, id: pl().id, enabled: on }).then(() => plugins_()))}
                  />
                }
              />
            )}
          </Show>
          <SelectSetting
            part="accounts.detail.model"
            title={t('accounts.detail.model')}
            description={
              <Show
                when={modelProblem(defaultsOf(id).model, models(), h().checking === true)}
                fallback={t('accounts.detail.modelHint', { name: h().name })}
              >
                {bad(t('accounts.detail.modelUnknown', { model: defaultsOf(id).model, name: h().name }))}
              </Show>
            }
            source={{ value: () => defaultsOf(id).model, set: (v) => setDefaults(id, { model: v, label: models().find((m) => m.id === v)?.label ?? v }) }}
            items={[
              { value: '', label: runs() ? t('accounts.detail.defaultIs', { model: runs() ?? '' }) : t('accounts.detail.default') },
              ...models().map((m) => ({ value: m.id, label: m.label })),
              // a kept model it no longer lists stays visible, so the error has something to point at
              ...(modelProblem(defaultsOf(id).model, models(), h().checking === true) ? [{ value: defaultsOf(id).model, label: defaultsOf(id).model }] : []),
            ]}
          />
          <Show when={efforts().length > 0}>
            <SelectSetting
              part="accounts.detail.effort"
              title={t('accounts.detail.effort')}
              description={t('accounts.detail.effortHint')}
              source={{ value: () => defaultsOf(id).effort, set: (v) => setDefaults(id, { effort: v }) }}
              items={[{ value: '', label: t('accounts.detail.default') }, ...efforts().map((e) => ({ value: e, label: e.charAt(0).toUpperCase() + e.slice(1) }))]}
            />
          </Show>
          <SelectSetting
            part="accounts.detail.mode"
            title={t('accounts.detail.mode')}
            description={t('accounts.detail.modeHint')}
            source={{ value: () => defaultsOf(id).mode, set: (v) => setDefaults(id, { mode: v }) }}
            items={[{ value: '', label: t('accounts.detail.modeAsPicked') }, ...MODES_FOR_HARNESS.map((m) => ({ value: m, label: t(`accounts.detail.mode.${m}`) }))]}
          />
          <BrowserDetail id={id} name={h().name} can={det()?.agent.browser ?? BROWSER_CAN_UNKNOWN} />
        </SettingsGroup>
        {/* what other plugins add to a harness (wisp.budget: where automations stop on it) */}
        <Slot input="shell.settings:harness" id={id} layout="contents" />
      </>
    );
  };

  /** A harness's own browser over General's: what it can't take is greyed out, with why; a harness without per-run MCP
   *  (Cursor, Antigravity) has none, said so, never shown as on. */
  const BrowserDetail = (p: { readonly id: string; readonly name: string; readonly can: BrowserCan }): JSX.Element => {
    const own = useSignal(browserEach.value);
    const all = useSignal(browserAll.value);
    const row = () => browserRow(p.can, browserOwn(own(), p.id) || browserGlobal(all()));
    const said = (): string => {
      const n = row().note;
      return n ? t(`accounts.browser.why.${n}`, { name: p.name }) : t('accounts.detail.browserHint', { name: p.name });
    };
    return (
      <SelectSetting
        part="accounts.detail.browser"
        title={t('accounts.detail.browser')}
        description={said()}
        disabled={row().note === 'noMcp'}
        source={{
          value: () => (row().note === 'noMcp' ? 'off' : browserOwn(own(), p.id)),
          set: (v) => browserEach.set(withBrowser(own(), p.id, v as BrowserValue | '')),
        }}
        items={[
          { value: '', label: t('accounts.browser.asGeneral', { value: t(`accounts.browser.${browserGlobal(all())}`) }) },
          ...row()
            .choices.filter((x) => x.id !== 'default')
            .map((x) => ({ value: x.id, label: t(`accounts.browser.${x.id}`), ...(x.why && { disabled: true }) })),
        ]}
      />
    );
  };

  /** General: what isn't a harness's own (GitHub and git, Lean runs, Ask before a changed harness, updates, Re-check). */
  const General = (): JSX.Element => (
    <>
      <SettingsGroup part="accounts.tools">
        {/* git only while it's missing; GitHub always (v1: "For pushing branches and opening pull requests") */}
        <For each={tools().filter((x) => x.id === 'gh' || !x.installed)}>
          {(x) => (
            <SettingsRow
              part="accounts.tool"
              title={x.id === 'gh' ? t('accounts.tool.github') : x.name}
              icon={x.id === 'gh' ? 'github' : 'branch'}
              description={x.installed ? t('accounts.tool.gh') : <HowTo tip={x.setup?.tip ?? t('accounts.tool.missing')} command={x.setup?.command} onCopy={copy} />}
              control={
                <Inline gap="3">
                  <Show when={!x.installed}>
                    <Docs name={x.id === 'gh' ? t('accounts.tool.ghCli') : x.name} url={x.setup?.url} />
                  </Show>
                  <Show when={x.id === 'gh' && x.installed}>{signControl(() => ({ id: 'gh', signedIn: login('gh')?.signedIn ?? x.signedIn }), '', false)}</Show>
                  <Show when={!x.installed && toolState(x)}>
                    {(s) => (
                      <Pill tone={s().tone} dot variant="plain" part="accounts.tool.state">
                        {s().label}
                      </Pill>
                    )}
                  </Show>
                </Inline>
              }
            />
          )}
        </For>
        <Show when={tools().some((x) => x.id === 'gh' && x.installed)}>
          <Steps c={c} agent="gh" account="" login={login('gh')} />
        </Show>
      </SettingsGroup>
      <SettingsGroup part="accounts.watch">
        {/* the browser agents drive (`runtime.browser`, ADR-0128); a harness's detail sets its own */}
        <SelectSetting
          part="settings.browser"
          title={t('accounts.browser.title')}
          description={t('accounts.browser.description')}
          source={text(browserAll, 'default')}
          items={BROWSER_VALUES.map((v) => ({ value: v, label: t(`accounts.browser.${v}`) }))}
        />
        {/* v1 Lean Claude Code runs (`runtime.leanRuns`): off, wisp's side runs, every run */}
        <SelectSetting
          part="settings.leanRuns"
          title={t('accounts.lean.title')}
          description={t('accounts.lean.description')}
          source={text(fromRow(c, 'runtime.leanRuns'), 'on')}
          items={(['off', 'on', 'all'] as const).map((v) => ({ value: v, label: t(`accounts.lean.${v}`) }))}
        />
        <SettingsRow
          part="settings.harnessAsk"
          title={t('accounts.ask.title')}
          description={t('accounts.ask.description')}
          control={<Switch aria-label={t('accounts.ask.title')} checked={place()?.ask ?? false} onChange={(on) => quiet(c.run(setAsk, { peer, on }))} />}
        />
      </SettingsGroup>
      {/* v1: Installed something new? Re-check, last */}
      <SettingsGroup part="accounts.recheck.group">
        {/* TRY-17: two or more with a newer version: all of them in one press */}
        <Show when={outdated().length > 1}>
          <SettingsRow
            part="accounts.updateAll"
            title={t('accounts.updateAll.title')}
            description={outdated()
              .map((h) => t('accounts.updateAll.item', { name: h.name, version: h.latest ?? '' }))
              .join(' · ')}
            control={
              <Button size="sm" variant="primary" icon="update" onClick={() => updateAll(outdated())}>
                {t('accounts.updateAll', { count: outdated().length })}
              </Button>
            }
          />
        </Show>
        <SettingsRow
          part="accounts.recheck"
          title={t('accounts.recheck.title')}
          description={t('accounts.recheck.hint')}
          control={
            <Button size="sm" variant="ghost" icon="replay" disabled={rechecking()} onClick={recheckNow}>
              {rechecking() ? t('accounts.recheck.running') : t('accounts.recheck')}
            </Button>
          }
        />
      </SettingsGroup>
    </>
  );

  return (
    <>
      <SettingsMasterDetail
        part="accounts.master"
        label={t('accounts.pane')}
        items={items()}
        selected={shown() ?? ''}
        onSelect={setPicked}
        search={{ placeholder: t('accounts.search'), value: query(), onInput: setQuery }}
        empty={t('accounts.search.none')}
      >
        <Show when={shown()} keyed fallback={<SettingsEmpty part="accounts.search.empty" icon="search" title={t('accounts.search.none')} description={t('accounts.search.noneHint')} />}>
          {(id) => (id === GENERAL ? <General /> : <HarnessDetail id={id} />)}
        </Show>
      </SettingsMasterDetail>
      <Show when={keysOf()}>{(h) => <Keys c={c} h={h()} keys={keysFor(h().id)} onClose={() => setKeysOf(undefined)} />}</Show>
      <Show when={adding()}>{(h) => <AddAccount brand={h().brand ?? h().name} onAdd={(label, kind) => addAnother(h(), label, kind)} onClose={() => setAdding(undefined)} />}</Show>
      <Show when={renaming()}>
        {(a) => <RenameAccount label={a().label} onRename={(label) => quiet(c.run(updateAccount, { peer, agent: a().agent, account: accOf(a()), label }))} onClose={() => setRenaming(undefined)} />}
      </Show>
      <Show when={removing()}>
        {(a) => <RemoveAccount label={a().label} onRemove={() => quiet(c.run(removeAccount, { peer, agent: a().agent, account: accOf(a()) }))} onClose={() => setRemoving(undefined)} />}
      </Show>
    </>
  );
}
