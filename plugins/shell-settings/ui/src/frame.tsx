// The Settings overlay (v1 openSettings): every plugin's SettingsPaneDef in one frame, grouped and
// ordered, one pane at a time. What's open is navigation: `params.pane` (so a toast's "Settings"
// link, Back and a reload land on the same pane); on a phone no pane means the list, and an
// explicit pane opens straight to it, as in v1. Each pane renders with its own plugin's client and
// inside an ErrorBoundary, so one broken pane never takes Settings down.

import { ErrorBoundary, Show, createMemo, createSignal, type JSX } from 'solid-js';
import { Dynamic } from 'solid-js/web';
import type { PluginClient, Vars } from '@wisp/client';
import { usePluginClient, useShell, type SettingsPaneDef } from '@wisp/shell';
import { Callout, EmptyState, Input, SettingsFrame, useSignal } from '@wisp/ui';
import { t as own } from './i18n.ts';
import { VIEW_ID, type SettingsParams } from './types.ts';
import { pickPane, settingsIntent, type PaneEntry } from './nav.ts';
import { filterSections, offeredPanes, sectionedPanes } from './phone-nav.ts';
import './i18n/lazy.ts';

/** What the frame reads: every registered pane and a translator (titles are other plugins' keys). */
export interface FrameDeps {
  readonly panes: () => readonly PaneEntry[];
  readonly t: (key: string, vars?: Vars) => string;
  /** the phone apps: no keyboard-only pane (v1 F-set-6), and the list's Back names the Tasks screen under it */
  readonly phone?: boolean;
  /** the list and the pane show side by side (wide): picking a pane replaces Settings' place in history (TRY-26) */
  readonly sideBySide?: () => boolean;
}


/** Inside the shell: its panes (each with its plugin's client) and its translator. */
export function shellDeps(): FrameDeps {
  const shell = useShell();
  const panes = useSignal(shell.settingsPanes());
  const clientOf = (plugin: string): PluginClient => usePluginClient(plugin);
  const entries = createMemo(() => panes().map((pane) => ({ pane, client: clientOf((pane as SettingsPaneDef & { plugin?: string }).plugin ?? 'shell') })));
  return { panes: entries, t: (k, v) => shell.t(k, v), phone: shell.platform.phone, sideBySide: () => shell.nav().layout === 'split' };
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
  const groups = createMemo(() => filterSections(all(), query(), ledeOf));
  const onSearchKey = (e: KeyboardEvent): void => {
    if (e.key === 'Enter') {
      const first = groups()[0]?.items[0]?.id;
      if (first !== undefined) props.client.navigate(settingsIntent(first, undefined, deps.sideBySide?.() === true));
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
  const current = createMemo(() => pickPane(panes(), order(), props.params.pane));
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
      onPick={(id) => props.client.navigate(settingsIntent(id, undefined, deps.sideBySide?.() === true))}
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
      <Show when={current()} keyed fallback={<EmptyState compact icon="settings" title={own('shell-settings.empty')} />}>
        {(entry) => (
          <ErrorBoundary
            fallback={() => (
              <Callout tone="bad" icon="warn">
                {own('shell-settings.pane-failed')}
              </Callout>
            )}
          >
            <Dynamic component={entry.pane.component} client={entry.client} {...(props.params.filter !== undefined && { filter: props.params.filter })} />
          </ErrorBoundary>
        )}
      </Show>
    </SettingsFrame>
  );
}
