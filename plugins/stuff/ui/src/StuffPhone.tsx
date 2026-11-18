// My Stuff in the phone apps (ADR-0065 step 14, design/mobile.md §A.2 My Stuff): the Stuff destination as the
// platform's screen, the kit's StuffPhone (the root bar with Search, Add files and ⋮ / ⋯; Android's filter chips or
// iOS's segmented control for the kinds; tiles or rows). What's kept, its search, kinds and groups are the browser
// page's (model.ts); every action is the browser page's own (page-run.ts) plus Share, the platform's share sheet. A
// list row swipes right to Share and left to Remove; both are in the row's ⋯ on screen. An app launches inside wisp,
// in its sandboxed frame over the whole screen (F-stuff-10). Its own chunk: only the phone apps load it.
import { createMemo, createSignal, onCleanup, Show, type JSX } from 'solid-js';
import { ownerOf, type PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { StuffPhone, StuffPhoneApp, type MenuEntry, type StuffPhoneGroup } from '@wisp/ui';
import { itemActions, type ActionId } from './actions.ts';
import { phoneImage, phoneThumb } from './card.ts';
import { publishTargets } from './commands.ts';
import { useEntries, useTaskTitle, type Entry } from './items.ts';
import { kindOf } from './kinds.ts';
import { cardMeta, computerOf, groupItems, withComputer, kindChips, shownItems, when, type KindFilter } from './model.ts';
import { pageRun } from './page-run.ts';
import { PaneCards, pinsOf } from './pins.tsx';
import { stuffState } from './state.ts';
import { TOPIC, type Item } from './types.ts';
import './i18n/lazy.ts';

/** What the phone screen carries out: the browser page's, less Show in folder (no Finder on a phone), plus Share. */
export const PHONE_ACTIONS: ReadonlySet<ActionId> = new Set(['open', 'share', 'pin-task', 'unpin-task', 'pin-home', 'unpin-home', 'stop', 'publish', 'pubcopy', 'pubopen', 'update', 'unpublish', 'copy', 'download', 'star', 'task', 'remove']);

/** The ⋮ / ⋯ menu's choices. */
const MAKE = 'make';
const BY_TIME = 'by-time';
const BY_TASK = 'by-task';
const GRID = 'grid';
const LIST = 'list';

/** how long Remove's Undo stays: the toast's time, then the removal runs (v1: commands.ts) */
export const UNDO_MS = 5000;

type Timer = ReturnType<PluginClient['scope']['timers']['setTimeout']>;
/** the removals waiting for their Undo, per plugin client (they outlive the screen) */
const PENDING = new WeakMap<PluginClient, Map<string, Timer>>();
const pendingOf = (c: PluginClient): Map<string, Timer> => {
  let m = PENDING.get(c);
  if (!m) PENDING.set(c, (m = new Map()));
  return m;
};

/** an app's frame: v1's pane sandbox (HomePanes) */
const APP_SANDBOX = 'allow-scripts allow-forms allow-same-origin';

export default function StuffPhonePage(props: { readonly client: PluginClient }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const t = (k: string, v?: Readonly<Record<string, string | number>>): string => c.t(k, v);
  const scope = c.scope.child('stuff-phone');
  onCleanup(() => scope.dispose());
  c.watch(c.home, TOPIC, scope);
  const entries = useEntries(c);
  const pins = pinsOf(c);
  const st = stuffState(c);
  c.run(publishTargets, { peer: c.home }).then(
    (ts) => st.setPublishTo(ts.map((x) => x.host).filter(Boolean)),
    () => {},
  );
  const [q, setQ] = createSignal('');
  const [kind, setKind] = createSignal<KindFilter>('all');
  const [byTask, setByTask] = createSignal(false);
  const [list, setList] = createSignal(false);
  // an app launched: over the whole screen, in its frame (Close / Done brings My Stuff back)
  const [app, setApp] = createSignal<{ readonly title: string; readonly url: string }>();
  const { run, upload } = pageRun({ c, shell, pins, entries, launched: (e, url) => setApp({ title: e.it.title || e.it.name, url }) });
  const taskTitle = useTaskTitle(c);
  // rows are keyed by entity key, never the bare item id: two computers can each hold an `st1` (r298), and keyed by id
  // one copy stood for both (its name, its menu, its Remove)
  const byKey = createMemo(() => new Map<string, Entry>(entries().map((e) => [e.key, e])));
  const byItem = createMemo(() => new Map(entries().map((e) => [e.it, e])));
  const filter = () => ({ query: q(), kind: kind(), starred: false });
  const chips = createMemo(() => kindChips(entries().map((e) => e.it), filter(), t));
  const shown = createMemo(() => shownItems(entries().map((e) => e.it), { ...filter(), kind: chips().kind }, t));
  const actionsOf = (e: Entry, items: readonly Item[]): MenuEntry[] =>
    itemActions(e.it, { ...(st.publishTo()[0] && { publishTo: st.publishTo()[0] }), pinnedTask: pins.pinned(e.it, 'task', items), pinnedHome: pins.pinned(e.it, 'home', items), task: !!e.it.from?.session, mobile: true })
      .filter((a) => PHONE_ACTIONS.has(a.id))
      .map((a) => ({ value: a.id, label: t(a.label, a.vars), icon: a.icon, ...(a.danger && { danger: true }) }));
  const groups = createMemo((): StuffPhoneGroup[] => {
    const now = Date.now();
    const items = entries().map((x) => x.it);
    // two computers' things in one list: each says whose it is, by the computer's own name (never "This Mac")
    const computer = computerOf(entries().map((x) => ownerOf(x.key)), (p) => c.peers.name(ownerOf(p)));
    return groupItems(shown(), byTask() ? 'task' : 'time', {
      t,
      now,
      task: taskTitle,
      month: (at, withYear) => new Date(at).toLocaleDateString(undefined, withYear ? { month: 'long', year: 'numeric' } : { month: 'long' }),
    }).map((g) => ({
      // its words are its key: a group appearing above another never shifts the others' (nothing rebuilds)
      id: g.label,
      label: g.label,
      items: g.items.flatMap((it) => {
        const e = byItem().get(it);
        const image = phoneImage(it);
        const thumb = image ? undefined : phoneThumb(it);
        return e
          ? [{ id: e.key, title: it.title || it.name, sub: withComputer(cardMeta(it, t), computer(ownerOf(e.key))), when: when(it.at, now), icon: kindOf(it.kind).icon, ...(image && { image }), ...(thumb && { thumb }), menu: actionsOf(e, items) }]
          : [];
      }),
    }));
  });
  // Remove (the menu's, or a swipe to the left): the thing leaves at once, with Undo on the toast; the removal runs once
  // the toast has gone (v1). Kept for the plugin's lifetime (its scope's timers), not this screen's: Undo still works
  // after switching tabs, and nothing stays hidden and kept
  const pending = pendingOf(c);
  const unhide = (key: string): void => {
    st.setHidden((h) => new Set([...h].filter((k) => k !== key)));
  };
  const removeSoon = (e: Entry): void => {
    if (pending.has(e.key)) return;
    st.setHidden((h) => new Set([...h, e.key]));
    const timer = c.scope.timers.setTimeout(() => {
      pending.delete(e.key);
      run(e, 'remove');
      unhide(e.key);
    }, UNDO_MS);
    pending.set(e.key, timer);
    shell.toast.show(t('stuff.phone.removed', { name: e.it.title || e.it.name }), {
      duration: UNDO_MS,
      action: {
        label: t('stuff.phone.undo'),
        onClick: () => {
          const tm = pending.get(e.key);
          if (tm === undefined) return;
          c.scope.timers.clearTimeout(tm);
          pending.delete(e.key);
          unhide(e.key);
        },
      },
    });
  };
  const act = (id: string, action: string): void => {
    const e = byKey().get(id);
    if (!e) return;
    if (action === 'remove') removeSoon(e);
    else run(e, action as ActionId);
  };
  let picker: HTMLInputElement | undefined;
  const menu = (): MenuEntry[] => [
    { value: MAKE, label: t('stuff.make.title'), icon: 'app' },
    { kind: 'separator' },
    { value: BY_TIME, label: t('stuff.page.byTime'), icon: 'clock', checked: !byTask() },
    { value: BY_TASK, label: t('stuff.page.byTask'), icon: 'feed', checked: byTask() },
    { kind: 'separator' },
    { value: GRID, label: t('stuff.page.grid'), icon: 'grid', checked: !list() },
    { value: LIST, label: t('stuff.page.list'), icon: 'menu', checked: list() },
  ];
  const onMenu = (v: string): void => {
    if (v === MAKE) c.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'stuff.make' as never, params: {} } });
    else if (v === BY_TIME || v === BY_TASK) setByTask(v === BY_TASK);
    else if (v === GRID || v === LIST) setList(v === LIST);
  };
  return (
    <>
      <input ref={(el) => (picker = el)} type="file" multiple hidden onChange={(ev) => upload(ev.currentTarget.files)} />
      <StuffPhone
        part="stuff.phone"
        title={t('stuff.title')}
        add={{ label: t('stuff.page.add'), onSelect: () => picker?.click() }}
        actionsLabel={t('stuff.phone.actions')}
        menu={{ label: t('stuff.phone.more'), items: menu(), onSelect: onMenu }}
        search={{ value: q(), label: t('stuff.page.searchLabel'), placeholder: t('stuff.page.search'), onInput: setQ }}
        kinds={{
          label: t('stuff.phone.kinds'),
          items: chips().chips.map((ch) => ({ id: ch.id, label: ch.id === 'all' ? t('stuff.kind.all') : t(kindOf(ch.id).many), count: ch.count })),
          value: chips().kind,
          onPick: (id) => setKind(id as KindFilter),
        }}
        view={list() ? 'list' : 'grid'}
        groups={groups()}
        onOpen={(id) => act(id, 'open')}
        onMenu={act}
        menuLabel={(name) => t('stuff.page.more', { name })}
        swipe={{
          right: { label: t('stuff.act.share'), icon: 'share', tone: 'accent' },
          left: { label: t('stuff.act.remove'), icon: 'trash', tone: 'bad' },
          onSwipe: (id, dir) => act(id, dir === 'right' ? 'share' : 'remove'),
        }}
        empty={entries().length > 0 ? undefined : { title: t('stuff.page.empty.title'), body: t('stuff.page.empty.description'), action: t('stuff.page.add'), onAction: () => picker?.click() }}
        nothing={t('stuff.page.nothing')}
        t={(k, v) => shell.t(`kit.${k}`, v)}
      />
      <PaneCards client={c} />
      <Show when={app()}>
        {(a) => <StuffPhoneApp title={a().title} src={a().url} sandbox={APP_SANDBOX} closeLabel={t('stuff.phone.close')} doneLabel={t('stuff.phone.done')} onClose={() => setApp(undefined)} />}
      </Show>
    </>
  );
}
