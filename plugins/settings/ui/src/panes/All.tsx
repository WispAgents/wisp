// Settings → All settings: every setting wisp's plugins declare, the same registry `wisp settings` and a
// formula's `settings:` reach, searchable, grouped by the plugin that owns it. Each row has the control its type
// takes (a switch, a choice, a number or text), shows its default when unset, and says what locks it (a formula).
import { createEffect, createMemo, createResource, createSignal, For, on, Show, type JSX } from 'solid-js';
import type { Json, PluginClient } from '@wisp/client';
import { Input, Select, SettingsGroup, SettingsRow, Switch, Text, useProgressive, useSignal } from '@wisp/ui';
import { list, type SettingInfo } from '../commands.ts';
import { t } from '../i18n.ts';
import { fromRow } from '../source.ts';
import '../i18n/lazy.ts';
import { ownerTitle, titleParts } from '../words.ts';
import { AUTO_SETTING } from '@wisp/shell';

/** rows built in the frame the pane opens; as many again each frame after */
const FIRST_ROWS = 16;

const words = (v: unknown): string => (v === null || v === undefined ? '' : typeof v === 'string' ? v : JSON.stringify(v));

function Control(props: { readonly c: PluginClient; readonly s: SettingInfo }): JSX.Element {
  const src = fromRow(props.c, props.s.key, (props.s.decl.default ?? null) as Json);
  const value = useSignal(src.value);
  const lockedBy = src.lockedBy ? useSignal(src.lockedBy) : (): string | undefined => undefined;
  const locked = (): boolean => !!props.s.lockedBy || !!lockedBy();
  const label = titleParts(props.s.decl.title ?? props.s.key).label;
  const [draft, setDraft] = createSignal<string | undefined>();
  const commit = (raw: string): void => {
    setDraft(undefined);
    if (props.s.decl.type === 'number' || props.s.decl.type === 'int') {
      const n = Number(raw);
      if (raw.trim() !== '' && Number.isFinite(n)) src.set(n);
    } else if (props.s.decl.type === 'json') {
      try {
        src.set(JSON.parse(raw) as Json);
      } catch {
        // not JSON: the box keeps what was typed until it is
        setDraft(raw);
      }
    } else src.set(raw);
  };
  switch (props.s.decl.type) {
    case 'boolean':
    case 'bool':
      return <Switch aria-label={label} checked={value() === true || value() === 'on'} disabled={locked()} onChange={(on) => src.set(on)} />;
    case 'enum':
      return <Select aria-label={label} size="sm" items={(props.s.decl.values ?? []).map((v) => ({ value: v, label: v }))} value={words(value()) || undefined} placeholder={t('settings.all.default')} disabled={locked()} onChange={(v) => src.set(v)} />;
    default:
      return <Input size="sm" mono aria-label={label} type={props.s.decl.secret ? 'password' : 'text'} value={draft() ?? words(value())} disabled={locked()} onInput={setDraft} onBlur={() => draft() !== undefined && commit(draft() ?? '')} onKeyDown={(e) => e.key === 'Enter' && commit(draft() ?? words(value()))} />;
  }
}

export default function All(props: { readonly client: PluginClient; /** Settings' search opened it on one setting (its key) */ readonly filter?: string }): JSX.Element {
  const c = props.client;
  const [rows] = createResource(() => c.run(list, { peer: c.home }).catch((): SettingInfo[] => []));
  const [q, setQ] = createSignal(props.filter ?? '');
  // Settings' search opening it again on another setting while it's open (the same pane is kept)
  createEffect(on(() => props.filter, (f) => f !== undefined && setQ(f), { defer: true }));
  const groups = createMemo(() => {
    const needle = q().trim().toLowerCase();
    const by = new Map<string, SettingInfo[]>();
    for (const s of rows() ?? []) {
      // Auto's switch isn't a control here while Auto is put away (owner, 2026-11-17): `wisp settings set runtime.auto true`
      if (s.key === AUTO_SETTING) continue;
      if (needle && !`${s.key} ${s.decl.title ?? ''} ${s.owner}`.toLowerCase().includes(needle)) continue;
      by.set(s.owner, [...(by.get(s.owner) ?? []), s]);
    }
    return [...by.entries()].sort(([a], [b]) => a.localeCompare(b));
  });
  // every row's control at once was the press's whole frame (perf r2 #9: 160 ms at 4×): a screenful first, the rest
  // over the frames after
  const total = createMemo(() => groups().reduce((n, [, items]) => n + items.length, 0));
  const upTo = useProgressive(total, FIRST_ROWS);
  // where each group's rows start in the whole list: a group draws its rows up to `upTo`, the ones it has kept
  const starts = createMemo(() => {
    let n = 0;
    return groups().map(([, items]) => ((n += items.length), n - items.length));
  });
  return (
    <>
      <Text as="p" tone="muted" part="settings.lede">
        {t('settings.all.lede')}
      </Text>
      <Input icon="search" aria-label={t('settings.all.search')} placeholder={t('settings.all.search')} value={q()} onInput={setQ} />
      <Show when={groups().length > 0} fallback={<Text tone="muted">{rows.loading ? t('settings.all.loading') : t('settings.all.none')}</Text>}>
        <For each={groups()}>
          {([owner, items], i) => (
            <Show when={upTo() > (starts()[i()] ?? 0)}>
            <SettingsGroup part="settings.all.group" title={ownerTitle(owner)}>
              <For each={items.slice(0, upTo() - (starts()[i()] ?? 0))}>
                {(s) => (
                  <SettingsRow
                    part="settings.all.row"
                    title={titleParts(s.decl.title ?? s.key).label}
                    description={
                      <>
                        <Show when={titleParts(s.decl.title ?? '').help}>
                          {(help) => <Text as="div" tone="muted">{help()}</Text>}
                        </Show>
                        <Text as="div" mono tone="muted">
                          {s.lockedBy ? t('settings.all.locked', { key: s.key, by: s.lockedBy }) : s.key}
                        </Text>
                      </>
                    }
                    wideControl={s.decl.type !== 'boolean' && s.decl.type !== 'bool'}
                    control={<Control c={c} s={s} />}
                  />
                )}
              </For>
            </SettingsGroup>
            </Show>
          )}
        </For>
      </Show>
    </>
  );
}
