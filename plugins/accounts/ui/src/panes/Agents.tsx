// Settings → Agents (v1 settings.js paintAgents, agent-login.js; design/v1-reference/settings.agents): the
// harnesses wisp found, each with its state, version and models; Claude and Codex with their extra accounts
// ("Add another … account"); a CLI sign-in's steps (the page to open, the code to paste); a key for harnesses
// that take one (opencode); Install for one wisp can install; the notice when a harness's program changed;
// git and the GitHub CLI that Ship it needs; Re-check. Every write shows at once.
import { createMemo, createSignal, For, Match, onCleanup, Show, Switch as Pick, type JSX } from 'solid-js';
import { entityKey, type PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
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
  SettingsGroup,
  SettingsInset,
  SettingsInsetAdd,
  SettingsInsetRow,
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
import { addAccount, install, keyRemove, keySet, loginCancel, loginInput, loginStart, logout, recheck, removeAccount, setAsk, trust, upgrade } from '../commands.ts';
import { t } from '../i18n.ts';
import { ACCOUNT, DETECT_TOPIC, HARNESS, KEY, LOGIN, PLACE, TOOL, TOPIC, loginId, type AccountRow, type HarnessRow, type KeyRow, type LoginRow, type PlaceRow, type ToolRow } from '../types.ts';
import { accountLine, changeTip, changeTitle, changeWhat, harnessLine, harnessState, howToGet, loginControl, masked, mergeHarnesses, present, recheckSummary, site, sentence, stillChanged, toolState } from '../words.ts';
import '../i18n/lazy.ts';

const quiet = (p: Promise<unknown>): void => {
  p.catch(() => {}); // the shell's onError shows the rollback toast
};

type Rows<T> = () => readonly T[];

function useRows<T>(c: PluginClient, kind: string): Rows<T> {
  const list = useSignal(c.entities.list<T>(kind));
  return () => list() as readonly T[];
}

/** The keys a harness signs in with (opencode Zen, Go…): set, test, remove; never the key itself. */
function Keys(props: { readonly c: PluginClient; readonly h: HarnessRow; readonly keys: readonly KeyRow[]; readonly onClose: () => void }): JSX.Element {
  const [typed, setTyped] = createSignal<Record<string, string>>({});
  const peer = props.c.home;
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && props.onClose()}
      title={t('accounts.keys.title', { name: props.h.name })}
      description={t('accounts.keys.where', { site: site(props.keys[0]?.url ?? '') })}
      part="accounts.keys"
    >
      <Stack gap="1">
        <For each={props.keys}>
          {(k) => (
            <SettingsRow
              part="accounts.key"
              title={k.name}
              description={k.set ? t('accounts.keys.saved', { hint: masked(k) }) : t('accounts.keys.none')}
              wideControl
              control={
                <Inline gap="2">
                  <Input
                    size="sm"
                    mono
                    type="password"
                    aria-label={t('accounts.keys.input', { name: k.name })}
                    placeholder={k.set ? t('accounts.keys.replace') : t('accounts.keys.paste')}
                    value={typed()[k.provider] ?? ''}
                    onInput={(v) => setTyped({ ...typed(), [k.provider]: v })}
                  />
                  <Button
                    size="sm"
                    disabled={(typed()[k.provider] ?? '').trim() === ''}
                    onClick={() => {
                      quiet(props.c.run(keySet, { peer, agent: k.agent, provider: k.provider, key: (typed()[k.provider] ?? '').trim() }));
                      setTyped({ ...typed(), [k.provider]: '' });
                    }}
                  >
                    {t('accounts.keys.save')}
                  </Button>
                  <Show when={k.set}>
                    <Button size="sm" variant="ghost" onClick={() => quiet(props.c.run(keyRemove, { peer, agent: k.agent, provider: k.provider }))}>
                      {t('accounts.keys.remove')}
                    </Button>
                  </Show>
                </Inline>
              }
            />
          )}
        </For>
      </Stack>
    </Dialog>
  );
}

/** Not found here: this OS's tip, the command to copy (v1 setup.js setupCommand), and its install guide. */
function HowTo(props: { readonly tip: string; readonly command?: string | undefined; readonly onCopy: (cmd: string) => void }): JSX.Element {
  return (
    <>
      {props.tip}
      <Show when={props.command}>{(cmd) => <CopyCommand part="accounts.setup.command" command={cmd()} label={t('accounts.setup.copy')} onCopy={props.onCopy} />}</Show>
    </>
  );
}

/** v1 setupDocs: the install guide, a ghost button that opens it outside wisp. */
function Docs(props: { readonly name: string; readonly url?: string | undefined }): JSX.Element {
  const shell = useShell();
  return (
    <Show when={props.url}>
      {(url) => (
        <Button size="sm" variant="ghost" icon="external" part="accounts.setup.docs" title={t('accounts.setup.guide', { name: props.name })} onClick={() => shell.platform.openExternal(url())}>
          {t('accounts.setup.docs')}
        </Button>
      )}
    </Show>
  );
}

export default function Agents(props: { readonly client: PluginClient }): JSX.Element {
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
  const harnesses = createMemo(() => mergeHarnesses(mine(), detected()));
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
  // keyed by id: a harness that settles updates its row in place (no remount, nothing moves)
  const byId = createMemo(() => new Map(harnesses().map((h) => [h.id, h])));
  const ids = createMemo(() => harnesses().map((h) => h.id), undefined, { equals: (a, b) => a.length === b.length && a.every((x, i) => x === b[i]) });
  const login = (agent: string, account = ''): LoginRow | undefined => logins().find((l) => l.agent === agent && loginId(l.agent, l.account) === loginId(agent, account));
  const accs = (agent: string): AccountRow[] =>
    accounts()
      .filter((a) => a.agent === agent)
      .sort((a, b) => a.ord - b.ord);
  const keysFor = (agent: string): KeyRow[] => keys().filter((k) => k.agent === agent);

  /** `row`: the harness's row as it is now (it updates in place as its detect settles) */
  const signControl = (row: () => HarnessRow, account: string, isAccount: boolean): JSX.Element => {
    const ctl = (): ReturnType<typeof loginControl> => loginControl(login(row().id, account), { account: isAccount, out: row().signedIn === false });
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

  const addAnother = (h: HarnessRow): void => {
    const n = accs(h.id).length + 1;
    quiet(c.run(addAccount, { peer, agent: h.id, id: `a${Date.now().toString(36)}`, label: t('accounts.account.n', { brand: h.brand ?? h.name, n }), kind: 'personal' }));
  };

  return (
    <>
      {/* v1 .sx-agents: one row per harness, lines between; its accounts in a box under it; then GitHub (and git while it's missing) */}
      <SettingsGroup part="accounts.harnesses">
        <For each={ids()}>
          {(id) => {
            // the last row it had: a harness that goes (its plugin switched off) may be read once more before its row unmounts
            let last = byId().get(id) as HarnessRow;
            const h = (): HarnessRow => (last = byId().get(id) ?? last);
            // its own Sign in (one account): the state says what that login says, never Ready beside Sign in
            const ownLogin = (): boolean => h().signIn === 'cli' && present(h()) && (!h().brand || accs(h().id).length <= 1);
            const state = () => harnessState(h(), ownLogin() ? (login(h().id, '')?.signedIn ?? undefined) : undefined);
            return (
              <>
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
                <Steps c={c} agent={h().id} account="" login={login(h().id)} />
                <Show when={h().brand && present(h())}>
                  <SettingsInset part="settings.accounts">
                    <For each={accs(h().id).length > 1 ? accs(h().id) : []}>
                      {(a) => (
                        <SettingsInsetRow
                          part="accounts.account"
                          title={a.label}
                          description={accountLine(a)}
                          state={a.state === 'limit' ? a.why || t('accounts.state.limit') : a.state === 'signedOut' ? t('accounts.state.signedOut') : undefined}
                          control={
                            <Inline gap="2">
                              {signControl(h, a.id, true)}
                              <Show when={!a.default}>
                                <Menu
                                  trigger={{ icon: 'dots', 'aria-label': t('accounts.account.more', { name: a.label }), variant: 'ghost', size: 'sm' }}
                                  items={[
                                    { value: 'out', label: t('accounts.login.signOut'), icon: 'lock' },
                                    { kind: 'separator' },
                                    { value: 'remove', label: t('accounts.account.remove'), icon: 'trash', danger: true },
                                  ]}
                                  onSelect={(v) => quiet(v === 'out' ? c.run(logout, { peer, agent: h().id, account: a.id }) : c.run(removeAccount, { peer, agent: h().id, account: a.id }))}
                                />
                              </Show>
                            </Inline>
                          }
                        />
                      )}
                    </For>
                    <SettingsInsetAdd part="accounts.add" label={t('accounts.account.add', { brand: h().brand ?? h().name })} onClick={() => addAnother(h())} />
                  </SettingsInset>
                </Show>
              </>
            );
          }}
        </For>
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
                  {/* gh signed in or out, as Home's tip asks (QA r1 #23: the row said nothing) */}
                  <Show when={toolState(x)}>
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
      </SettingsGroup>
      <SettingsGroup part="accounts.watch">
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
              <Button size="sm" variant="primary" icon="update" onClick={() => update(outdated())}>
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
            <Button
              size="sm"
              variant="ghost"
              icon="replay"
              disabled={rechecking()}
              onClick={() => {
                setRechecking(true);
                // v1 recheckSummary: what this Re-check turned up, here and on every machine (the server asks them too)
                const before = new Map(harnesses().map((h) => [h.id, h]));
                c.run(recheck, { peer })
                  .then((after) => shell.toast.show(recheckSummary(before, after ?? [])))
                  .catch(() => {})
                  .finally(() => setRechecking(false));
              }}
            >
              {rechecking() ? t('accounts.recheck.running') : t('accounts.recheck')}
            </Button>
          }
        />
      </SettingsGroup>
      <Show when={keysOf()}>{(h) => <Keys c={c} h={h()} keys={keysFor(h().id)} onClose={() => setKeysOf(undefined)} />}</Show>
    </>
  );
}
