// My Stuff (v1 web/js/features/stuff.js, design/v1-reference/stuff): what you and your agents kept. A search
// over titles, notes, #tags and tasks; the kinds as chips; grouped by time (Today, This week, months) or by
// task; cards with their kind, title and facts, each with its menu (open or launch, copy, download, star,
// the task it came from, show in folder, remove). Add files drops them here. The rows are the peer's
// `stuff.item` view rows on the `stuff` topic; every write shows at once.
import { createMemo, createSignal, For, lazy, onCleanup, Show, type JSX } from 'solid-js';
import type { EntityKey, PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { Button, Card, Icon, Inline, Menu, Stack, StuffFrame, Text, Wrap } from '@wisp/ui';
import { itemActions, type ActionId } from './actions.ts';
import { publishTargets } from './commands.ts';
import { pageRun } from './page-run.ts';
import { stuffState } from './state.ts';
import { PaneCards, pinsOf } from './pins.tsx';
import { useEntries, useTaskTitle, type Entry } from './items.ts';
import { kindOf } from './kinds.ts';
import { cardMeta, groupItems, kindChips, shownItems, when, type KindFilter } from './model.ts';
import { TOPIC } from './types.ts';
import './i18n/lazy.ts';

/** The actions this page carries out; the rest (pins, edit) live where they're done. */
const HERE: ReadonlySet<ActionId> = new Set(['open', 'pin-task', 'unpin-task', 'pin-home', 'unpin-home', 'stop', 'publish', 'pubcopy', 'pubopen', 'update', 'unpublish', 'copy', 'download', 'star', 'task', 'folder', 'remove']);

// the phone apps (data-platform android | ios): My Stuff as the platform's screen (ADR-0065 step 14); its own chunk
const StuffPhonePage = lazy(() => import('./StuffPhone.tsx'));

/** The browser (a phone's too) keeps v1's page; the phone apps draw the platform's screen in its place. */
export default function StuffPage(props: { readonly client: PluginClient }): JSX.Element {
  const shell = useShell();
  return (
    <Show when={shell.platform.phone} fallback={<BrowserPage client={props.client} />}>
      <StuffPhonePage client={props.client} />
    </Show>
  );
}

function BrowserPage(props: { readonly client: PluginClient }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const t = (k: string, v?: Readonly<Record<string, string | number>>): string => c.t(k, v);
  const scope = c.scope.child('stuff-page');
  onCleanup(() => scope.dispose());
  c.watch(c.home, TOPIC, scope);
  const entries = useEntries(c);
  const pins = pinsOf(c);
  // where an app can be published: the grids this computer is on (none: no Publish)
  const st = stuffState(c);
  c.run(publishTargets, { peer: c.home }).then(
    (ts) => st.setPublishTo(ts.map((x) => x.host).filter(Boolean)),
    () => {},
  );
  const [q, setQ] = createSignal('');
  const [kind, setKind] = createSignal<KindFilter>('all');
  // v1's sort and layout: by time or by the task each came from; cards or rows
  const [byTask, setByTask] = createSignal(false);
  const [list, setList] = createSignal(false);
  const taskTitle = useTaskTitle(c);
  const byKey = createMemo(() => new Map(entries().map((e) => [e.it.id, e])));
  const filter = () => ({ query: q(), kind: kind(), starred: false });
  const chips = createMemo(() => kindChips(entries().map((e) => e.it), filter(), t));
  const shown = createMemo(() => shownItems(entries().map((e) => e.it), { ...filter(), kind: chips().kind }, t));
  const groups = createMemo(() =>
    groupItems(shown(), byTask() ? 'task' : 'time', {
      t,
      now: Date.now(),
      task: taskTitle,
      month: (at, withYear) => new Date(at).toLocaleDateString(undefined, withYear ? { month: 'long', year: 'numeric' } : { month: 'long' }),
    }),
  );
  const { run, upload } = pageRun({ c, shell, pins, entries });
  let picker: HTMLInputElement | undefined;

  const card = (e: Entry): JSX.Element => {
    const k = kindOf(e.it.kind);
    const actions = () => itemActions(e.it, { ...(st.publishTo()[0] && { publishTo: st.publishTo()[0] }), pinnedTask: pins.pinned(e.it, 'task', entries().map((x) => x.it)), pinnedHome: pins.pinned(e.it, 'home', entries().map((x) => x.it)), task: !!e.it.from?.session, mobile: false }).filter((a) => HERE.has(a.id));
    return (
      <Card part="stuff.card" as="li" interactive onClick={() => run(e, 'open')}>
        <Stack gap="2">
          <Inline gap="2" justify="between">
            <Inline gap="2">
              <Icon name={k.icon} />
              <Text weight="medium" truncate>
                {e.it.title || e.it.name}
              </Text>
            </Inline>
            <Menu
              part="stuff.card.menu"
              trigger={{ icon: 'dots', 'aria-label': t('stuff.page.more', { name: e.it.title || e.it.name }), variant: 'ghost', size: 'xs' }}
              items={actions().map((a) => ({ value: a.id, label: t(a.label, a.vars), icon: a.icon, ...(a.danger && { danger: true }) }))}
              onSelect={(v) => run(e, v as ActionId)}
            />
          </Inline>
          <Show when={e.it.excerpt}>
            <Text size="sm" tone="muted" truncate>
              {e.it.excerpt}
            </Text>
          </Show>
          <Inline gap="2" justify="between">
            <Text size="sm" tone="muted" truncate>
              {cardMeta(e.it, t)}
            </Text>
            <Text size="sm" tone="muted">
              {when(e.it.at, Date.now())}
            </Text>
          </Inline>
        </Stack>
      </Card>
    );
  };

  const total = (): string => (entries().length > 0 ? t('stuff.page.count', { count: entries().length }) : '');
  return (
    <StuffFrame
      title={t('stuff.title')}
      role={t('stuff.role')}
      count={total()}
      actions={[
        { id: 'makePane', icon: 'app', label: t('stuff.make.title'), title: t('stuff.make.headTitle'), onClick: () => c.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'stuff.make' as never, params: {} } }) },
        { id: 'add', icon: 'plus', label: t('stuff.page.add'), title: t('stuff.page.addTitle'), onClick: () => picker?.click() },
      ]}
      close={{ label: t('stuff.page.close'), onClick: () => c.navigate({ t: 'home' }) }}
      search={{ value: q(), placeholder: t('stuff.page.search'), label: t('stuff.page.searchLabel'), onInput: setQ }}
      kinds={
        <Show when={chips().chips.length > 1}>
          <For each={chips().chips}>
            {(ch) => (
              <Button size="sm" variant={chips().kind === ch.id ? 'primary' : 'ghost'} onClick={() => setKind(ch.id)}>
                {`${ch.id === 'all' ? t('stuff.kind.all') : t(kindOf(ch.id).many)} ${ch.count}`}
              </Button>
            )}
          </For>
        </Show>
      }
      sort={{ icon: byTask() ? 'feed' : 'clock', label: byTask() ? t('stuff.page.byTask') : t('stuff.page.byTime'), onClick: () => setByTask(!byTask()) }}
      view={{ list: list(), gridLabel: t('stuff.page.grid'), listLabel: t('stuff.page.list'), onPick: setList }}
      empty={
        entries().length > 0
          ? undefined
          : {
              title: t('stuff.page.empty.title'),
              keep: [t('stuff.page.empty.keep1'), t('stuff.page.empty.keep2')],
              asks: [t('stuff.page.empty.ask1'), t('stuff.page.empty.ask2'), t('stuff.page.empty.ask3'), t('stuff.page.empty.ask4'), t('stuff.page.empty.ask5')],
              mention: [t('stuff.page.empty.mention1'), '@', t('stuff.page.empty.mention2')],
              or: t('stuff.page.empty.or'),
              add: t('stuff.page.empty.add'),
              onAdd: () => picker?.click(),
            }
      }
    >
      <input ref={(el) => (picker = el)} type="file" multiple hidden onChange={(ev) => upload(ev.currentTarget.files)} />
      <Show when={shown().length > 0} fallback={<Text tone="muted">{t('stuff.page.nothing')}</Text>}>
        <For each={groups()}>
          {(g) => (
            <Stack gap="2" part="stuff.group">
              <Text size="sm" tone="muted" weight="medium">
                {g.label}
              </Text>
              <Show
                when={list()}
                fallback={
                  <Wrap gap="3" part="stuff.cards">
                    <For each={g.items}>{(it) => (byKey().get(it.id) ? card(byKey().get(it.id) as Entry) : null)}</For>
                  </Wrap>
                }
              >
                <Stack gap="2" part="stuff.cards">
                  <For each={g.items}>{(it) => (byKey().get(it.id) ? card(byKey().get(it.id) as Entry) : null)}</For>
                </Stack>
              </Show>
            </Stack>
          )}
        </For>
      </Show>
      <PaneCards client={c} />
    </StuffFrame>
  );
}

export type { EntityKey };
