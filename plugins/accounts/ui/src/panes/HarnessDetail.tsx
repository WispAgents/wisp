// size-ok: the five sections of one harness, each a short function; split further they would only pass props along
// Settings → Agents, the harness picked (owner, settings audit r1): what used to be its one row, in sections. Account &
// auth (its state, Sign in or its keys, its accounts), Defaults (the model and level a new task on it starts with),
// Limits & budget (its own Ask near the limit, and the usage rows every harness shares), Binary (the program wisp
// pinned, its version and Update, Re-detect, the changed-program notice) and Advanced (its plugin's switch, the
// update check, the changed-program ask, and Claude Code's lean runs and subagent model). Loaded when a harness is
// picked, not with the list.
import { For, Show, createSignal, type JSX } from 'solid-js';
import { Button, Callout, ChoiceSetting, Inline, Menu, Pill, Select, SelectSetting, SettingsGroup, SettingsInset, SettingsInsetAdd, SettingsInsetRow, SettingsRow, Switch, SwitchSetting, useSignal, Wrap } from '@wisp/ui';
import { install, setAsk, setPluginEnabled, trust, updateAccount, logout, redetect } from '../commands.ts';
import { defaultsOf, effortsFor, holdOf, HOLDS, pluginOf, staleModel, withEffort, withHold, withModel, type Hold } from '../harness-pane.ts';
import { t } from '../i18n.ts';
import { fromRow, text } from '../source.ts';
import type { SettingSource } from '@wisp/ui';
import type { Json } from '@wisp/client';
import type { AccountRow, HarnessRow } from '../types.ts';
import { accOf, accountLine, accountMenu, accountState, changeTip, changeTitle, changeWhat, harnessLine, harnessState, howToGet, present, sentence, stillChanged } from '../words.ts';
import { Docs, HowTo, SignControl } from './AgentBits.tsx';
import { quiet, type AgentsData } from './agents-data.ts';
import { Steps } from './Steps.tsx';
import '../i18n/lazy.ts';

/** A bool setting as a switch, written as true / false (v1's 'on' / 'off' read as such). */
function bool(src: SettingSource<Json>, dflt: boolean): SettingSource<boolean> {
  const v = (): boolean => {
    const x = (src.value as () => unknown)();
    return x === true || x === 'on' || (x !== false && x !== 'off' && dflt);
  };
  return { value: v, ...(src.lockedBy && { lockedBy: src.lockedBy }), set: (on) => src.set(on) };
}

export interface DetailProps {
  readonly d: AgentsData;
  readonly h: () => HarnessRow;
  readonly onKeys: (h: HarnessRow) => void;
  readonly onAdd: (h: HarnessRow) => void;
  readonly onRename: (a: AccountRow) => void;
  readonly onRemove: (a: AccountRow) => void;
}

export default function HarnessDetail(props: DetailProps): JSX.Element {
  return (
    <>
      <Auth {...props} />
      <Show when={!props.h().disabled}>
        <Defaults {...props} />
        <Limits {...props} />
        <Binary {...props} />
      </Show>
      <Advanced {...props} />
    </>
  );
}

/** Its state, its sign-in or keys, how to get it while it's missing, and its accounts (v1 .sx-agents row, .sx-accts). */
function Auth(props: DetailProps): JSX.Element {
  const d = props.d;
  const h = props.h;
  const state = () => d.state(h());
  /** v1 accounts.js moreMenu, every action on its account (the row's account is after its harness in its id) */
  const onAccount = (a: AccountRow, v: string): void => {
    const args = { peer: d.peer, agent: a.agent, account: accOf(a) };
    if (v === 'rename') props.onRename(a);
    if (v === 'kind') quiet(d.c.run(updateAccount, { ...args, kind: a.kind === 'work' ? 'personal' : 'work' }));
    if (v === 'up' || v === 'down') quiet(d.c.run(updateAccount, { ...args, move: v }));
    if (v === 'out') quiet(d.c.run(logout, args));
    if (v === 'remove') props.onRemove(a);
  };
  return (
    <SettingsGroup part="accounts.auth" title={t('accounts.section.auth')}>
      <SettingsRow
        part="accounts.harness"
        title={h().name}
        {...(d.logoOf(h().id) && { logo: d.logoOf(h().id) as string })}
        description={
          <Show when={!h().disabled} fallback={t('accounts.disabled.line')}>
            <Show when={howToGet(h())} fallback={harnessLine(h(), d.accs(h().id).length)}>
              {(how) => <HowTo tip={how().tip} command={how().command} onCopy={d.copy} />}
            </Show>
          </Show>
        }
        control={
          <Wrap gap="3" align="center">
            <Show when={!h().disabled && h().signIn === 'key' && present(h())}>
              <Button size="sm" variant="primary" icon="lock" part="accounts.keys.open" onClick={() => props.onKeys(h())}>
                {d.keysFor(h().id).some((k) => k.set) ? t('accounts.keys.edit') : t('accounts.keys.add')}
              </Button>
            </Show>
            <Show when={d.ownLogin(h())}>
              <SignControl d={d} row={h} account="" isAccount={false} />
            </Show>
            <Show when={!h().disabled && !h().checking && !present(h()) && !h().installable}>
              <Docs name={h().name} url={h().setup?.url} />
            </Show>
            <Show when={!h().disabled && !h().checking && !present(h()) && h().installable === true}>
              <Button size="sm" disabled={h().installing === true} onClick={() => quiet(d.c.run(install, { peer: d.peer, agent: h().id }))}>
                {h().installing === true ? t('accounts.installing') : t('accounts.install')}
              </Button>
            </Show>
            <Pill tone={state().tone} dot variant="plain">
              {state().label}
            </Pill>
          </Wrap>
        }
      />
      {/* the harness's own sign-in, unless its accounts are listed: then it's under its account's row (the CLI's own
          login is the first account's, and showed twice) */}
      <Show when={!h().disabled && !(h().brand && present(h()) && d.accs(h().id).length > 1)}>
        <Steps c={d.c} agent={h().id} account="" login={d.login(h().id)} />
      </Show>
      <Show when={!h().disabled && h().brand && present(h())}>
        <SettingsInset part="settings.accounts">
          <For each={d.accs(h().id).length > 1 ? d.accs(h().id) : []}>
            {(a, i) => (
              <>
                <SettingsInsetRow
                  part="accounts.account"
                  title={a.label}
                  description={accountLine(a)}
                  state={accountState(a)}
                  control={
                    <Inline gap="2">
                      {/* signed out or not is the account's own (v1: its state), not its harness's */}
                      <SignControl d={d} row={() => ({ id: h().id, signedIn: a.state === 'signedOut' ? false : null })} account={accOf(a)} isAccount />
                      {/* v1: the same ⋯ on every row, what can't apply disabled (the CLI's own login: Sign out, not Remove) */}
                      <Menu
                        part="accounts.account.menu"
                        trigger={{ icon: 'dots', 'aria-label': t('accounts.account.more', { name: a.label }), variant: 'ghost', size: 'sm' }}
                        items={accountMenu(a, i(), d.accs(h().id).length)}
                        onSelect={(v) => onAccount(a, v)}
                      />
                    </Inline>
                  }
                />
                {/* its sign-in under its row (v1 loginPanel): the page to open, the code, the paste box */}
                <Steps c={d.c} agent={h().id} account={accOf(a)} login={d.login(h().id, accOf(a))} inset />
              </>
            )}
          </For>
          <SettingsInsetAdd part="accounts.add" label={t('accounts.account.add', { brand: h().brand ?? h().name })} onClick={() => props.onAdd(h())} />
        </SettingsInset>
      </Show>
    </SettingsGroup>
  );
}

/** The model and level a new task on it starts with: `runtime.launch`'s entries for this harness (typed, never JSON). */
function Defaults(props: DetailProps): JSX.Element {
  const d = props.d;
  const h = props.h;
  const launch = fromRow(d.c, 'runtime.launch', {});
  // followed (useSignal: the setting row is an alien signal; round 324): a pick from New task shows here at once
  const value = useSignal(launch.value as () => unknown);
  const saved = (): { model: string; effort: string } => defaultsOf(value(), h().id);
  const models = (): { value: string; label: string }[] => [
    { value: '', label: t('accounts.defaults.model.own') },
    ...h()
      .models.filter((m) => m.id !== '')
      .map((m) => ({ value: m.id, label: m.label })),
  ];
  const efforts = (): readonly string[] => effortsFor(h(), saved().model);
  const setModel = (id: string): void => {
    const label = h().models.find((m) => m.id === id)?.label ?? id;
    launch.set(withModel(value(), h().id, id, label) as never);
  };
  return (
    <SettingsGroup part="accounts.defaults" title={t('accounts.section.defaults')} description={t('accounts.section.defaults.description', { name: h().name })}>
      <SettingsRow
        part="accounts.defaults.model"
        settingKey="runtime.launch"
        title={t('accounts.defaults.model')}
        description={h().models.length === 0 ? t('accounts.defaults.model.none') : t('accounts.defaults.model.hint')}
        wideControl
        control={<Select size="sm" aria-label={t('accounts.defaults.model')} items={models()} value={saved().model} disabled={h().models.length === 0} onChange={setModel} />}
      />
      {/* a saved model it no longer lists (renamed, retired): say so where it's set, and put it back to its own */}
      <Show when={staleModel(saved().model, h().models)}>
        <Callout
          tone="warn"
          part="accounts.defaults.stale"
          action={
            <Button size="sm" onClick={() => setModel('')}>
              {t('accounts.defaults.reset')}
            </Button>
          }
        >
          {t('accounts.defaults.stale', { model: saved().model })}
        </Callout>
      </Show>
      <Show when={efforts().length > 0}>
        <SettingsRow
          part="accounts.defaults.effort"
          title={t('accounts.defaults.effort')}
          description={t('accounts.defaults.effort.hint')}
          wideControl
          control={
            <Select
              size="sm"
              aria-label={t('accounts.defaults.effort')}
              items={[{ value: '', label: t('accounts.defaults.effort.own') }, ...efforts().map((e) => ({ value: e, label: e }))]}
              value={saved().effort}
              onChange={(e) => launch.set(withEffort(value(), h().id, e) as never)}
            />
          }
        />
      </Show>
    </SettingsGroup>
  );
}

/** Its own Ask near the limit (`budget.limitHoldBy`, over everyone's), and the usage rows every harness shares. */
function Limits(props: DetailProps): JSX.Element {
  const d = props.d;
  const h = props.h;
  const by = fromRow(d.c, 'budget.limitHoldBy', {});
  const read = useSignal(by.value as () => unknown);
  const hold = { value: () => holdOf(read(), h().id) as string, ...(by.lockedBy && { lockedBy: by.lockedBy }), set: (v: string) => by.set(withHold(read(), h().id, v as Hold)) };
  const usage = (): void => d.c.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane: 'usage' } }, replace: true });
  return (
    <SettingsGroup part="accounts.limits" title={t('accounts.section.limits')}>
      <ChoiceSetting
        part="accounts.limits.hold"
        settingKey="budget.limitHoldBy"
        title={t('accounts.limits.hold')}
        description={t('accounts.limits.hold.description')}
        source={hold}
        items={HOLDS.map((v) => ({ value: v, label: v === 'same' ? t('accounts.limits.same') : v === 'off' ? t('accounts.limits.off') : `${v}%` }))}
      />
      <ChoiceSetting
        part="accounts.limits.silent"
        title={t('accounts.limits.silent')}
        description={t('accounts.limits.silent.description')}
        source={text(fromRow(d.c, 'budget.silentStop'), 'off')}
        items={(['off', '1', '2', '4', '8'] as const).map((v) => ({ value: v, label: v === 'off' ? t('accounts.limits.off') : t('accounts.limits.silent.hours', { count: Number(v) }) }))}
      />
      <SwitchSetting part="accounts.limits.wait" title={t('accounts.limits.wait')} description={t('accounts.limits.wait.description')} source={bool(fromRow(d.c, 'runtime.limitWait'), false)} />
      <SettingsRow
        part="accounts.limits.usage"
        title={t('accounts.limits.usage')}
        description={t('accounts.limits.usage.description')}
        control={
          <Button size="sm" variant="ghost" icon="stats" onClick={usage}>
            {t('accounts.limits.usage.open')}
          </Button>
        }
      />
    </SettingsGroup>
  );
}

/** The program wisp pinned (as found, and really), its version and Update, Re-detect, the changed-program notice. */
function Binary(props: DetailProps): JSX.Element {
  const d = props.d;
  const h = props.h;
  const [detecting, setDetecting] = createSignal(false);
  const busy = (): boolean => d.updating().has(h().id) || h().installing === true;
  const version = (): string =>
    !h().version ? t('accounts.binary.unknown') : h().latest ? t('accounts.binary.latest', { version: h().version ?? '', latest: h().latest ?? '' }) : t('accounts.binary.current', { version: h().version ?? '' });
  return (
    <SettingsGroup part="accounts.binary" title={t('accounts.section.binary')}>
      <SettingsRow
        part="accounts.binary.path"
        title={t('accounts.binary.path')}
        description={<Show when={h().pin} fallback={t('accounts.binary.unpinned')}>{(p) => <span title={p().real}>{t('accounts.binary.pinned', { path: p().path, real: p().real })}</span>}</Show>}
      />
      <SettingsRow
        part="accounts.binary.version"
        title={t('accounts.binary.version')}
        description={version()}
        control={
          <Show when={busy() || (!h().checking && present(h()) && h().latest)}>
            <Button size="sm" part="accounts.update" disabled={busy()} onClick={() => d.update([h()])}>
              {busy() ? t('accounts.updating') : t('accounts.update', { version: h().latest ?? '' })}
            </Button>
          </Show>
        }
      />
      <SettingsRow
        part="accounts.binary.redetect"
        title={t('accounts.binary.redetect')}
        description={t('accounts.binary.redetect.hint')}
        control={
          <Button
            size="sm"
            variant="ghost"
            icon="replay"
            disabled={detecting()}
            onClick={() => {
              setDetecting(true);
              d.c.run(redetect, { peer: d.peer, agent: h().id })
                .catch(() => {})
                .finally(() => setDetecting(false));
            }}
          >
            {detecting() ? t('accounts.binary.redetecting') : t('accounts.binary.redetect')}
          </Button>
        }
      />
      <Show when={stillChanged(h()) && h().changed}>
        {(ch) => (
          <SettingsRow
            part="accounts.changed"
            title={changeTitle(h().name, ch())}
            description={<span title={changeTip(ch())}>{t('accounts.change.whatWhy', { what: sentence(changeWhat(ch())) })}</span>}
            control={
              <Button size="sm" onClick={() => quiet(d.c.run(trust, { peer: d.peer, agent: h().id }))}>
                {t('accounts.change.trust')}
              </Button>
            }
          />
        )}
      </Show>
    </SettingsGroup>
  );
}

/** Its plugin's switch, the update check, the changed-program ask; Lean runs and the subagent model where it has them. */
function Advanced(props: DetailProps): JSX.Element {
  const d = props.d;
  const h = props.h;
  const plugin = () => pluginOf(h().id, d.plugins());
  // the harness that runs subagents of its own says so (HarnessUiDef.variants: Claude Code's), as Try it two ways reads it
  const claudeLike = (): boolean => d.markOf(h().id)?.variants?.includes('subagentModel') === true;
  return (
    <SettingsGroup part="accounts.advanced" title={t('accounts.section.advanced')}>
      <Show when={plugin()}>
        {(p) => (
          <SettingsRow
            part="accounts.advanced.enabled"
            title={t('accounts.advanced.enabled')}
            description={t('accounts.advanced.enabled.description')}
            control={
              <Switch
                aria-label={t('accounts.advanced.enabled')}
                checked={p().enabled}
                onChange={(on) =>
                  d.c
                    .run(setPluginEnabled, { peer: d.peer, id: p().id, enabled: on })
                    .then(() => d.rereadPlugins())
                    .catch(() => {})
                }
              />
            }
          />
        )}
      </Show>
      <Show when={claudeLike() && !h().disabled}>
        <GlobalClaude d={d} />
      </Show>
      <GlobalRows d={d} />
    </SettingsGroup>
  );
}

/** Claude Code's runs: lean (no telemetry, update checks or connectors) and the model its subagents run on. */
export function GlobalClaude(props: { readonly d: AgentsData }): JSX.Element {
  return (
    <>
      {/* v1 Lean Claude Code runs (`runtime.leanRuns`): off, wisp's side runs, every run */}
      <SelectSetting
        part="settings.leanRuns"
        settingKey="runtime.leanRuns"
        title={t('accounts.lean.title')}
        description={t('accounts.lean.description')}
        source={text(fromRow(props.d.c, 'runtime.leanRuns'), 'on')}
        items={(['off', 'on', 'all'] as const).map((v) => ({ value: v, label: t(`accounts.lean.${v}`) }))}
      />
      <SelectSetting
        part="settings.subagentModel"
        settingKey="runtime.subagentModel"
        title={t('accounts.advanced.subagent')}
        description={t('accounts.advanced.subagent.description')}
        source={text(fromRow(props.d.c, 'runtime.subagentModel'), 'task')}
        items={(['task', 'haiku', 'sonnet'] as const).map((v) => ({ value: v, label: t(`accounts.advanced.subagent.${v}`) }))}
      />
    </>
  );
}

/** All harnesses' shared rows: Claude Code's runs, then every harness's (the old pane's Lean runs and ask, kept here). */
export function SharedRows(props: { readonly d: AgentsData }): JSX.Element {
  return (
    <>
      <GlobalClaude d={props.d} />
      <GlobalRows d={props.d} />
    </>
  );
}

/** Every harness's: ask before running a changed program, and the daily check for newer versions. */
export function GlobalRows(props: { readonly d: AgentsData }): JSX.Element {
  const d = props.d;
  return (
    <>
      <SettingsRow
        part="settings.harnessAsk"
        title={t('accounts.ask.title')}
        description={t('accounts.ask.description')}
        control={<Switch aria-label={t('accounts.ask.title')} checked={d.place()?.ask ?? false} onChange={(on) => quiet(d.c.run(setAsk, { peer: d.peer, on }))} />}
      />
      <SwitchSetting
        part="settings.updateCheck"
        settingKey="accounts.updateCheck"
        title={t('accounts.advanced.updateCheck')}
        description={t('accounts.advanced.updateCheck.description')}
        source={bool(fromRow(d.c, 'accounts.updateCheck'), true)}
      />
    </>
  );
}
