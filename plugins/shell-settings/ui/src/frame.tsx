// The Settings overlay (v1 openSettings): every plugin's SettingsPaneDef in one frame, grouped and
// ordered, one pane at a time. What's open is navigation: `params.pane` (so a toast's "Settings"
// link, Back and a reload land on the same pane); on a phone no pane means the list, and an
// explicit pane opens straight to it, as in v1. Each pane renders with its own plugin's client and
// inside an ErrorBoundary, so one broken pane never takes Settings down.

import { ErrorBoundary, Show, createMemo, createResource, createSignal, type JSX } from 'solid-js';
import { Dynamic } from 'solid-js/web';
import { defineCommand, type PeerId, type PluginClient, type Vars } from '@wisp/client';
import { titleParts, usePluginClient, useShell, type SettingsPaneDef } from '@wisp/shell';
import { Callout, EmptyState, Input, SettingsFocus, SettingsFrame, Skeleton, Stack, useAfterPaint, useDerived, useSignal } from '@wisp/ui';
import { t as own } from './i18n.ts';
import { VIEW_ID, type SettingsParams } from './types.ts';
import { pickPane, samePane, settingsIntent, type PaneEntry } from './nav.ts';
import { filterSections, offeredPanes, sectionedPanes } from './phone-nav.ts';
import { foundHarnesses, foundSettings, harnessTarget, settingTarget, type FoundHarness, type FoundSetting, type RoutePane } from './search.ts';
import './i18n/lazy.ts';

/** What the frame reads: every registered pane and a translator (titles are other plugins' keys). */
export interface FrameDeps {
  readonly panes: () => readonly PaneEntry[];
  readonly t: (key: string, vars?: Vars) => string;
  /** the phone apps: no keyboard-only pane (v1 F-set-6), and the list's Back names the Tasks screen under it */
  readonly phone?: boolean;
  /** the list and the pane show side by side (wide): picking a pane replaces Settings' place in history (TRY-26) */
  readonly sideBySide?: () => boolean;
  /** the harnesses the plugins registered: each is a search result of its own, opening Settings → Agents on it */
  readonly harnesses?: () => readonly FoundHarness[];
}


/** every declared setting (wisp.settings' read, as All settings lists them): what the search finds besides panes */
const settingsList = defineCommand<{ peer: PeerId }, readonly { key: string; owner: string; decl: { title?: string | null } }[]>({
  id: 'settings.list',
  owner: (a) => a.peer,
  wire: () => ({}),
  optimistic: 'none',
  pending: 'the settings list',
});

/** Inside the shell: its panes (each with its plugin's client) and its translator. */
export function shellDeps(): FrameDeps {
  const shell = useShell();
  const panes = useSignal(shell.settingsPanes());
  const clientOf = (plugin: string): PluginClient => usePluginClient(plugin);
  const all = createMemo(() => panes().map((pane) => ({ pane, client: clientOf((pane as SettingsPaneDef & { plugin?: string }).plugin ?? 'shell') })));
  // a pane with an `if` shows only while it says so (Auto's while its switch is on, owner #37); useDerived, as it may
  // read the entity store
  const entries = useDerived(() => all().filter((e) => e.pane.if?.(e.client) ?? true));
  const harnesses = useSignal(shell.harnesses());
  return { panes: entries, t: (k, v) => shell.t(k, v), phone: shell.platform.phone, sideBySide: () => shell.nav().layout === 'split', harnesses };
}

export function SettingsView(props: { readonly client: PluginClient; readonly params: SettingsParams; readonly deps?: FrameDeps }): JSX.Element {
  const deps = props.deps ?? shellDeps();
  const groupLabel = (id: string): string => {
    const key = `shell-settings.group.${id}`;
    const s = deps.t(key);
    return s === key ? id : s;
  };
  const panes = createMemo(() => offeredPanes(deps.panes(), deps.phone === true));
  // sections by group everywhere (TRY-28): M3 headers / iOS grouped sections on phones, headed groups in the side list
  const all = createMemo(() => sectionedPanes(panes(), (k) => deps.t(k), groupLabel));
  const order = createMemo(() => all().flatMap((g) => g.items.map((i) => i.id)));
  // the search above the list (TRY-28): by a pane's name, its section and its lede
  const [query, setQuery] = createSignal('');
  const ledeOf = (id: string): string => {
    const title = panes().find((e) => e.pane.id === id)?.pane.title ?? '';
    const s = deps.t(`${title}.lede`);
    return s === `${title}.lede` ? '' : s;
  };
  // a pane's rows' words (`<title>.keywords`: "theme light dark …"), so "theme" finds General (audit A F11)
  const aboutOf = (id: string): string => {
    const title = panes().find((e) => e.pane.id === id)?.pane.title ?? '';
    const k = deps.t(`${title}.keywords`);
    return `${ledeOf(id)} ${k === `${title}.keywords` ? '' : k}`;
  };
  // every declared setting, read once a search starts, through All settings' plugin (wisp.settings owns the read)
  const reader = (): PluginClient | undefined => panes().find((e) => e.pane.id === 'all')?.client;
  // each pane's rows (its lazy `keys`), read with the settings so a found one opens where its row is
  const keysOf = new Map<string, readonly string[] | undefined>();
  const loadKeys = (): Promise<unknown> =>
    Promise.all(
      panes().map(async (e) => {
        const load = e.pane.keys;
        keysOf.set(e.pane.id, load === undefined ? undefined : await load().catch(() => undefined));
      }),
    );
  const [declared] = createResource(
    () => (query().trim() !== '' ? reader() : undefined),
    (c) =>
      Promise.all([
        c
          .run(settingsList, { peer: c.home })
          .then((rows) => rows.map((r): FoundSetting => ({ key: r.key, owner: r.owner, ...titleParts(r.decl.title ?? r.key) })))
          .catch((): FoundSetting[] => []),
        loadKeys(),
      ]).then(([found]) => found),
  );
  const paneOwners = (): RoutePane[] => panes().map((e) => ({ id: e.pane.id, plugin: (e.pane as { plugin?: string }).plugin ?? '', keys: keysOf.get(e.pane.id) }));
  // each harness its own result ("Claude Code — Agents"), while Agents is there to open on it
  const agentsTitle = (): string | undefined => {
    const p = panes().find((e) => e.pane.id === 'agents')?.pane;
    return p ? deps.t(p.title) : undefined;
  };
  const groups = createMemo(() => {
    const byPane = filterSections(all(), query(), aboutOf);
    const pane = agentsTitle();
    const hs = pane === undefined ? [] : foundHarnesses(deps.harnesses?.() ?? [], query());
    const found = foundSettings(declared() ?? [], query()).slice(0, 20);
    const out = [...byPane];
    if (hs.length > 0 && pane !== undefined)
      out.push({ id: 'harnesses', label: pane, items: hs.map((h) => ({ id: `harness:${h.id}`, label: own('shell-settings.search.harness', { name: h.name, pane }), icon: 'agent' as const })) });
    if (found.length > 0) out.push({ id: 'found', label: own('shell-settings.search.settings'), items: found.map((f) => ({ id: `setting:${f.key}`, label: f.label, icon: 'settings' as const })) });
    return out;
  });
  const open = (id: string): void => {
    const side = deps.sideBySide?.() === true;
    if (id.startsWith('harness:')) {
      const to = harnessTarget(id.slice('harness:'.length));
      return props.client.navigate(settingsIntent(to.pane, to.filter, side));
    }
    if (!id.startsWith('setting:')) return props.client.navigate(settingsIntent(id, undefined, side));
    const f = (declared() ?? []).find((x) => `setting:${x.key}` === id);
    if (!f) return;
    const to = settingTarget(f, paneOwners());
    props.client.navigate(settingsIntent(to.pane, to.filter, side, to.focus));
  };
  const nothing = (): boolean => query().trim() !== '' && !declared.loading && groups().length === 0;
  const onSearchKey = (e: KeyboardEvent): void => {
    if (e.key === 'Enter') {
      const first = groups()[0]?.items[0]?.id;
      if (first !== undefined) open(first);
    } else if (e.key === 'Escape' && query() !== '') {
      // the first Esc clears the search; the next closes Settings
      e.preventDefault();
      e.stopPropagation();
      setQuery('');
    }
  };
  const search = (
    <Input
      type="search"
      icon="search"
      size="sm"
      value={query()}
      onInput={setQuery}
      onKeyDown={onSearchKey}
      placeholder={own('shell-settings.search')}
      aria-label={own('shell-settings.search')}
      part="shell-settings.search"
    />
  );
  // the same pane keeps its entry: a pane list made again (a setting changed) doesn't remake the open pane
  const current = createMemo(() => pickPane(panes(), order(), props.params.pane), undefined, { equals: samePane });
  // the pane picked: its row lights and its title shows in the click's frame, its body is built in the frame after
  // (perf r2 #9: a pane opened again built in the click, All settings 160 ms and Usage 120 ms at 4×)
  const body = useAfterPaint(current, samePane);
  const building = (): boolean => !samePane(body(), current());
  // the phone apps' list: Back is titled with the screen under Settings (iOS), Tasks, whose me menu opens it
  const backLabel = (): string | undefined => {
    if (!deps.phone) return undefined;
    const s = deps.t('shell.dest.tasks');
    return s === 'shell.dest.tasks' ? undefined : s;
  };
  const close = (): void => props.client.navigate({ t: 'closeOverlay', id: VIEW_ID });
  const lede = (): string | undefined => {
    const c = current();
    if (!c) return undefined;
    const key = `${c.pane.title}.lede`;
    const s = deps.t(key);
    return s === key ? undefined : s;
  };
  const onKey = (e: KeyboardEvent): void => {
    if (e.key === 'Escape' && !e.defaultPrevented) close();
  };
  return (
    <SettingsFrame
      // v1's own name for the whole sheet (.modal.sx), without a namespace: parity measures it by this
      part={'settings' as `${string}.${string}`}
      rootPart={'settings.frame' as `${string}.${string}`}
      title={own('shell-settings.title')}
      groups={groups()}
      search={search}
      current={current()?.pane.id ?? ''}
      onPick={open}
      paneTitle={current() ? deps.t(current()?.pane.title ?? '') : own('shell-settings.title')}
      lede={lede()}
      view={props.params.pane === undefined ? 'list' : 'pane'}
      onUp={() => props.client.navigate(settingsIntent())}
      upLabel={own('shell-settings.up')}
      onClose={close}
      closeLabel={own('shell-settings.close')}
      backLabel={backLabel()}
      onKeyDown={onKey}
    >
      <Show when={!nothing()} fallback={<EmptyState compact icon="search" title={own('shell-settings.search.none', { q: query().trim() })} part="shell-settings.search.empty" />}>
      <Show when={!building()} fallback={<PaneSkeleton />}>
      <Show when={body()} keyed fallback={<EmptyState compact icon="settings" title={own('shell-settings.empty')} />}>
        {(entry) => (
          <ErrorBoundary
            fallback={() => (
              <Callout tone="bad" icon="warn">
                {own('shell-settings.pane-failed')}
              </Callout>
            )}
          >
            {/* the setting the search opened it on: its row (SettingsRow settingKey) scrolls to, flashes and takes focus */}
            <SettingsFocus setting={props.params.focus}>
              <Dynamic component={entry.pane.component} client={entry.client} {...(props.params.filter !== undefined && { filter: props.params.filter })} />
            </SettingsFocus>
          </ErrorBoundary>
        )}
      </Show>
      </Show>
      </Show>
    </SettingsFrame>
  );
}

/** Where a pane's rows will be, the frame before they're built. */
function PaneSkeleton(): JSX.Element {
  return (
    <Stack gap="4" part="shell-settings.pane.loading" aria-hidden="true">
      <Skeleton shape="block" width="2/3" height="bar" />
      <Skeleton width="full" />
      <Skeleton width="3/4" />
    </Stack>
  );
}
