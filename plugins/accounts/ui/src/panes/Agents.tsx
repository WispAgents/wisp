// Settings → Agents (v1 settings.js paintAgents, agent-login.js; design/v1-reference/settings.agents), as a list and the
// one picked (owner, settings audit r1: one flat box of every harness had no room for each one's own settings). The
// list: a filter, All harnesses (Update all, Re-check everything, git and GitHub, the rows every harness shares), then
// every harness wisp knows with its mark, a dot for its state (ready, signed out, missing, checking, disabled) and a
// ring of its fullest usage window. The pick is in the Settings params (`harness:<id>`), so a reload, Back and the
// search land on it. The harness's own sections (HarnessDetail.tsx) load when one is picked.
import { createMemo, createSignal, For, lazy, Show, Suspense, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { Button, EmptyState, Input, Inline, Pill, SettingsGroup, SettingsRow, SettingsSplit, Skeleton, Stack, useSettingsFocus, useSignal, type SettingsSplitItem } from '@wisp/ui';
import { addAccount, removeAccount, updateAccount } from '../commands.ts';
import { ALL, harnessFilter, lineOf, pickOf, ringTone, selectFilter, showsDetail, STATUS_TONE } from '../harness-pane.ts';
import { fromRow } from '../source.ts';
import { t } from '../i18n.ts';
import type { AccountKind, AccountRow, HarnessRow } from '../types.ts';
import { accOf, toolState } from '../words.ts';
import { AddAccount, RemoveAccount, RenameAccount } from './AccountDialogs.tsx';
import { Docs, HowTo, Keys, SignControl } from './AgentBits.tsx';
import { agentsData, quiet, type AgentsData } from './agents-data.ts';
import { Steps } from './Steps.tsx';
import '../i18n/lazy.ts';

const HarnessDetail = lazy(() => import('./HarnessDetail.tsx'));
// the rows every harness shares, on All harnesses too (Settings' search lands there on runtime.leanRuns and the like)
const Shared = lazy(() => import('./HarnessDetail.tsx').then((m) => ({ default: m.SharedRows })));

export default function Agents(props: { readonly client: PluginClient; readonly filter?: string }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const d = agentsData(c, shell);
  const [q, setQ] = createSignal('');
  const ids = createMemo(() => d.harnesses().map((h) => h.id), undefined, { equals: (a, b) => a.length === b.length && a.every((x, i) => x === b[i]) });
  const pick = (): string => pickOf(props.filter, ids());
  // the pick is navigation: it replaces Settings' place in history, so Back leaves Settings, not each harness picked.
  // Opening one (a click, Enter) shows its detail, narrow too; the list's keys only move the pick (`pick:<id>`)
  const go = (filter: string): void => c.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane: 'agents', filter } }, replace: true });
  const open = (id: string): void => go(harnessFilter(id));
  const select = (id: string): void => go(selectFilter(id));
  // the setting Settings' search asked for: its row is in the detail, so narrow the detail shows (All harnesses' shared
  // rows when no harness is named)
  const focus = useSettingsFocus();
  // each harness's own Ask-near-the-limit line over everyone's: its ring turns amber where its runs are asked about
  const holdAt = useSignal(fromRow(c, 'budget.limitHold', '90').value as () => unknown);
  const holdBy = useSignal(fromRow(c, 'budget.limitHoldBy', {}).value as () => unknown);
  // keyed by id: a harness that settles updates its row in place (no remount, nothing moves)
  const byId = createMemo(() => new Map(d.harnesses().map((h) => [h.id, h])));
  const items = createMemo((): SettingsSplitItem[] => {
    const words = q().toLowerCase().split(/\s+/).filter(Boolean);
    const all: SettingsSplitItem = { id: ALL, label: t('accounts.list.all'), icon: 'agent' };
    const list = d.harnesses()
      .filter((h) => words.every((w) => `${h.name} ${h.id}`.toLowerCase().includes(w)))
      .map((h): SettingsSplitItem => {
        const s = d.status(h);
        const w = d.ringOf(h.id);
        return {
          id: h.id,
          label: h.name,
          logo: d.logoOf(h.id),
          icon: 'agent',
          dot: { tone: STATUS_TONE[s], label: d.state(h).label },
          ring: w && { value: w.used, label: t('accounts.ring', { window: w.label, pct: Math.round(w.used * 100) }), tone: ringTone(w.used, lineOf(holdAt(), holdBy(), h.id)) },
        };
      });
    return words.length === 0 ? [all, ...list] : list;
  });
  const picked = (): HarnessRow | undefined => byId().get(pick());
  // the last row it had: a harness that goes (its plugin switched off and its row not back yet) reads once more
  let last: HarnessRow | undefined;
  const h = (): HarnessRow => (last = picked() ?? last) as HarnessRow;

  // the boxes a row opens: a harness's keys, an account's Add, Rename and Remove
  const [keysOf, setKeysOf] = createSignal<HarnessRow | undefined>();
  const [adding, setAdding] = createSignal<HarnessRow | undefined>();
  const [renaming, setRenaming] = createSignal<AccountRow | undefined>();
  const [removing, setRemoving] = createSignal<AccountRow | undefined>();
  // v1 addBox: a name (blank: "Claude 2"), Personal or Work and the notice, then its row and its sign-in at once
  const addAnother = (x: HarnessRow, label: string, kind: AccountKind): void => {
    const n = d.accs(x.id).length + 1;
    quiet(c.run(addAccount, { peer: d.peer, agent: x.id, id: `a${Date.now().toString(36)}`, label: label || t('accounts.account.n', { brand: x.brand ?? x.name, n }), kind }));
  };
  const filter = (
    <Input type="search" icon="search" size="sm" value={q()} onInput={setQ} placeholder={t('accounts.list.filter')} aria-label={t('accounts.list.filter')} part="accounts.list.filter" />
  );
  const loading = (
    <Stack gap="3" aria-hidden="true">
      <Skeleton shape="block" width="2/3" height="bar" />
      <Skeleton width="full" />
    </Stack>
  );
  return (
    <>
      <SettingsSplit
        part="accounts.split"
        label={t('accounts.list.label')}
        filter={filter}
        items={items()}
        current={pick()}
        onPick={select}
        onOpen={open}
        view={showsDetail(props.filter, focus()) ? 'detail' : 'list'}
        onBack={() => c.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane: 'agents' } }, replace: true })}
        backLabel={t('accounts.list.back')}
        detailTitle={picked()?.name ?? t('accounts.list.all')}
        empty={<EmptyState compact icon="search" title={t('accounts.list.none', { q: q().trim() })} />}
      >
        <Suspense fallback={loading}>
          <Show when={pick() !== ALL && picked() !== undefined} fallback={<AllHarnesses d={d} shared={<Shared d={d} />} />}>
            <HarnessDetail d={d} h={h} onKeys={setKeysOf} onAdd={setAdding} onRename={setRenaming} onRemove={setRemoving} />
          </Show>
        </Suspense>
      </SettingsSplit>
      <Show when={keysOf()}>{(x) => <Keys c={c} h={x()} keys={d.keysFor(x().id)} onClose={() => setKeysOf(undefined)} />}</Show>
      <Show when={adding()}>{(x) => <AddAccount brand={x().brand ?? x().name} onAdd={(label, kind) => addAnother(x(), label, kind)} onClose={() => setAdding(undefined)} />}</Show>
      <Show when={renaming()}>
        {(a) => <RenameAccount label={a().label} onRename={(label) => quiet(c.run(updateAccount, { peer: d.peer, agent: a().agent, account: accOf(a()), label }))} onClose={() => setRenaming(undefined)} />}
      </Show>
      <Show when={removing()}>
        {(a) => <RemoveAccount label={a().label} onRemove={() => quiet(c.run(removeAccount, { peer: d.peer, agent: a().agent, account: accOf(a()) }))} onClose={() => setRemoving(undefined)} />}
      </Show>
    </>
  );
}

/** All harnesses: Update all, git and GitHub (what Ship it needs), the rows every harness shares, and Re-check. */
function AllHarnesses(props: { readonly d: AgentsData; readonly shared: JSX.Element }): JSX.Element {
  const d = props.d;
  return (
    <>
      {/* TRY-17: two or more with a newer version: all of them in one press */}
      <Show when={d.outdated().length > 1}>
        <SettingsGroup part="accounts.updates">
          <SettingsRow
            part="accounts.updateAll"
            title={t('accounts.updateAll.title')}
            description={d
              .outdated()
              .map((h) => t('accounts.updateAll.item', { name: h.name, version: h.latest ?? '' }))
              .join(' · ')}
            control={
              <Button size="sm" variant="primary" icon="update" onClick={() => d.updateAll(d.outdated())}>
                {t('accounts.updateAll', { count: d.outdated().length })}
              </Button>
            }
          />
        </SettingsGroup>
      </Show>
      {/* git only while it's missing; GitHub always (v1: "For pushing branches and opening pull requests") */}
      <SettingsGroup part="accounts.harnesses" title={t('accounts.section.tools')}>
        <For each={d.tools().filter((x) => x.id === 'gh' || !x.installed)}>
          {(x) => (
            <SettingsRow
              part="accounts.tool"
              title={x.id === 'gh' ? t('accounts.tool.github') : x.name}
              icon={x.id === 'gh' ? 'github' : 'branch'}
              description={x.installed ? t('accounts.tool.gh') : <HowTo tip={x.setup?.tip ?? t('accounts.tool.missing')} command={x.setup?.command} onCopy={d.copy} />}
              control={
                <Inline gap="3">
                  <Show when={!x.installed}>
                    <Docs name={x.id === 'gh' ? t('accounts.tool.ghCli') : x.name} url={x.setup?.url} />
                  </Show>
                  {/* v1: GitHub signs in and out from its row (gh's web login: a code and a link, under it) */}
                  <Show when={x.id === 'gh' && x.installed}>
                    <SignControl d={d} row={() => ({ id: 'gh', signedIn: d.login('gh')?.signedIn ?? x.signedIn })} account="" isAccount={false} />
                  </Show>
                  {/* gh signed in or out, as Home's tip asks (QA r1 #23: the row said nothing) */}
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
        <Show when={d.tools().some((x) => x.id === 'gh' && x.installed)}>
          <Steps c={d.c} agent="gh" account="" login={d.login('gh')} />
        </Show>
      </SettingsGroup>
      <SettingsGroup part="accounts.watch">{props.shared}</SettingsGroup>
      {/* v1: Installed something new? Re-check, last */}
      <SettingsGroup part="accounts.recheck.group">
        <SettingsRow
          part="accounts.recheck"
          title={t('accounts.recheck.title')}
          description={t('accounts.recheck.hint')}
          control={
            <Button size="sm" variant="ghost" icon="replay" disabled={d.rechecking()} onClick={d.recheckAll}>
              {d.rechecking() ? t('accounts.recheck.running') : t('accounts.recheck')}
            </Button>
          }
        />
      </SettingsGroup>
    </>
  );
}
