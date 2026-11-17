// My Stuff (v1 web/js/features/stuff.js, design/v1-reference/stuff): what you and your agents kept. A search
// over titles, notes, #tags and tasks; the kinds as chips; grouped by time (Today, This week, months) or by
// task; v1's cards (a preview of the thing, its tools on hover, its facts) or rows; each with its menu (⋯ or a
// right-click). Opening one opens it inside wisp as v1 did: pictures in the viewer, docs and code in the reader, an
// app in its window. Add files, a drop or a paste keeps files here. The rows are the peer's `stuff.item` view rows on
// the `stuff` topic; every write shows at once.
import { createMemo, createSignal, For, lazy, onCleanup, onMount, Show, type JSX } from 'solid-js';
import type { EntityKey, PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { ContextMenu, StuffCard, StuffFrame, StuffGroup, StuffKind, StuffNone } from '@wisp/ui';
import { useSignal } from '@wisp/ui/pure';
import { itemActions, type ActionId } from './actions.ts';
import { cardLook, headCount, launchLook, noneWhat, thumbOf } from './card.ts';
import { publishTargets, updateItem } from './commands.ts';
import { pageRun, type Shown } from './page-run.ts';
import { removeWithUndo } from './remove.ts';
import { openedByPress } from './opened.ts';
import { stuffState } from './state.ts';
import { PaneCards, pinsOf } from './pins.tsx';
import { useEntries, useTaskTitle, type Entry } from './items.ts';
import { kindOf } from './kinds.ts';
import { canCopy, canDownload, fmtSize, groupItems, kindChips, pastedName, shownItems, type KindFilter } from './model.ts';
import { TOPIC } from './types.ts';
import { EditDetails, Windows } from './Windows.tsx';
import './i18n/lazy.ts';

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

// size-ok: the page's state (search, kind, picks, what's open) is shared by its head, its shelf and its windows
function BrowserPage(props: { readonly client: PluginClient }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const t = (k: string, v?: Readonly<Record<string, string | number>>): string => c.t(k, v);
  const scope = c.scope.child('stuff-page');
  onCleanup(() => scope.dispose());
  c.watch(c.home, TOPIC, scope);
  const entries = useEntries(c);
  const pins = pinsOf(c);
  const st = stuffState(c);
  // where an app can be published: the grids this computer is on (none: no Publish)
  c.run(publishTargets, { peer: c.home }).then(
    (ts) => st.setPublishTo(ts.map((x) => x.host).filter(Boolean)),
    () => {},
  );
  const [q, setQ] = createSignal('');
  const [starred, setStarred] = createSignal(false);
  // v1's picks, kept in this browser: the kind, by time or task, cards or rows
  const picks = useSignal(st.picks.value);
  const pick = (p: Partial<{ kind: KindFilter; group: 'time' | 'task'; layout: 'grid' | 'list' }>): void => st.picks.set((cur) => ({ ...cur, ...p }));
  const byTask = (): boolean => picks().group === 'task';
  const list = (): boolean => picks().layout === 'list';
  const [shown, setShown] = createSignal<Shown | undefined>();
  const [editing, setEditing] = createSignal<{ e: Entry; anchor: HTMLElement | undefined } | undefined>();
  const [menuFor, setMenuFor] = createSignal<Entry | undefined>();
  const taskTitle = useTaskTitle(c);
  const items = () => entries().map((e) => e.it);
  const byKey = createMemo(() => new Map(entries().map((e) => [e.it.id, e])));
  const filter = () => ({ query: q(), kind: picks().kind, starred: starred() });
  const chips = createMemo(() => kindChips(items(), filter(), t));
  const visible = createMemo(() => shownItems(items(), { ...filter(), kind: chips().kind }, t));
  const groups = createMemo(() =>
    groupItems(visible(), byTask() ? 'task' : 'time', {
      t,
      now: Date.now(),
      task: taskTitle,
      month: (at, withYear) => new Date(at).toLocaleDateString(undefined, withYear ? { month: 'long', year: 'numeric' } : { month: 'long' }),
    }),
  );
  const entryOf = (id: string): Entry | undefined => byKey().get(id);
  const { run, upload } = pageRun({
    c,
    shell,
    pins,
    entries,
    show: setShown,
    edit: (e) => setEditing({ e, anchor: anchorOf(e) }),
    anchor: (e) => anchorOf(e),
    removeSoon: (e, now) => removeWithUndo(c, shell, e, now),
    media: () => visible().flatMap((it) => (entryOf(it.id) ? [entryOf(it.id) as Entry] : [])),
  });
  let picker: HTMLInputElement | undefined;
  let root: HTMLElement | undefined;
  // what a card's action came from (v1 act's `btn`): its ⋯ while it shows (the menu was opened from it), else the card
  // (a card held by its Allow or Edit card keeps its ⋯ shown: see `held`)
  function anchorOf(e: Entry): HTMLElement | undefined {
    const card = root?.querySelector<HTMLElement>(`[data-kit='st-card'][data-id="${CSS.escape(e.it.id)}"]`) ?? undefined;
    return card?.querySelector<HTMLElement>("[data-kit='st-more']") ?? card;
  }

  // ⋯ and a right-click: v1's itemMenu, every action the page has
  const menuItems = () => {
    const e = menuFor();
    if (!e) return [];
    const all = items();
    return itemActions(e.it, { ...(st.publishTo()[0] && { publishTo: st.publishTo()[0] }), pinnedTask: pins.pinned(e.it, 'task', all), pinnedHome: pins.pinned(e.it, 'home', all), task: !!(e.it.from?.session && taskTitle(e.it.from.session) !== undefined), mobile: false, prevWhen: e.it.prev ? new Date(e.it.prev.at).toLocaleDateString(undefined, { month: 'short', day: 'numeric' }) : '' }).map((a) => ({
      value: a.id,
      label: t(a.label, a.vars),
      icon: a.icon,
      ...(a.danger && { danger: true }),
    }));
  };
  const openMenu = (e: Entry, b: HTMLButtonElement): void => {
    setMenuFor(e);
    const r = b.getBoundingClientRect();
    // v1: under the button, its right edge near the button's, kept inside the window (clampToViewport)
    const x = Math.max(8, Math.min(r.right - 200, window.innerWidth - 208));
    b.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: x, clientY: r.bottom + 4 }));
  };

  // the keys on a card (v1 onKey): Enter or Space opens, Delete removes, S stars, the arrows go through the cards
  const onCardKey = (e: Entry, ev: KeyboardEvent): void => {
    const card = ev.currentTarget as HTMLElement;
    if (ev.target !== card) return;
    if (ev.key === 'Enter' || ev.key === ' ') return (ev.preventDefault(), run(e, 'open'));
    if (ev.key === 'Delete' || ev.key === 'Backspace') {
      ev.preventDefault();
      const next = (card.nextElementSibling ?? card.previousElementSibling) as HTMLElement | null;
      run(e, 'remove');
      return next?.focus();
    }
    if (ev.key === 's' && !ev.metaKey && !ev.ctrlKey) return (ev.preventDefault(), run(e, 'star'));
    const step = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: 0, ArrowUp: 0 }[ev.key];
    if (step === undefined) return;
    ev.preventDefault();
    const cards = [...(root?.querySelectorAll<HTMLElement>("[data-kit='st-card']") ?? [])];
    const i = cards.indexOf(card);
    const cols = list() ? 1 : Math.max(1, Math.round((card.parentElement?.clientWidth ?? 1) / (card.offsetWidth || 1)));
    const by = ev.key === 'ArrowDown' ? cols : ev.key === 'ArrowUp' ? -cols : step;
    if (ev.key === 'ArrowUp' && i < cols) return search()?.focus();
    cards[Math.max(0, Math.min(cards.length - 1, i + by))]?.focus();
  };
  const nav = useSignal(shell.nav);
  const stacked = (): boolean => nav().layout === 'stack';
  const search = (): HTMLInputElement | null => root?.querySelector<HTMLInputElement>("[data-kit='sf-search'] input") ?? null;

  onMount(() => {
    const box = search();
    // Escape clears the search; ↓ goes to the cards (v1)
    box?.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape' && box.value) {
        ev.stopPropagation();
        setQ('');
      } else if (ev.key === 'ArrowDown') {
        ev.preventDefault();
        root?.querySelector<HTMLElement>("[data-kit='st-card']")?.focus();
      }
    });
    // v1 openStuff: opened (the button, a toast's Open, ⌘K) puts the caret in the search; the page as it was left,
    // back after a reload, doesn't. A press opened it when the page still has the user's activation from it.
    if (openedByPress() && !matchMedia('(pointer: coarse)').matches) requestAnimationFrame(() => box?.focus({ preventScroll: true }));
    // '/' searches while the page is open; a pasted file (a screenshot) is kept
    const onKey = (ev: KeyboardEvent): void => {
      if (ev.key !== '/' || ev.metaKey || ev.ctrlKey || (ev.target as Element | null)?.closest?.('input, textarea, select, [contenteditable="true"]')) return;
      ev.preventDefault();
      ev.stopImmediatePropagation();
      search()?.focus();
    };
    const onPaste = (ev: ClipboardEvent): void => {
      if ((ev.target as Element | null)?.closest?.('input, textarea, [contenteditable="true"]')) return;
      const files = [...(ev.clipboardData?.files ?? [])];
      if (!files.length) return;
      ev.preventDefault();
      const now = new Date();
      upload(files.map((f, i) => new File([f], pastedName(f.name, f.type, now, i), { type: f.type })));
    };
    document.addEventListener('keydown', onKey, true);
    document.addEventListener('paste', onPaste);
    onCleanup(() => {
      document.removeEventListener('keydown', onKey, true);
      document.removeEventListener('paste', onPaste);
    });
  });

  // the card whose Allow or Edit card is up: its tools stay (StuffCard `held`)
  const modal = useSignal(st.modal);
  const held = (id: string): boolean => editing()?.e.it.id === id || (modal()?.kind === 'allow' && menuFor()?.it.id === id);
  const card = (e: Entry): JSX.Element => {
    const it = e.it;
    const look = () => cardLook(it, t, Date.now());
    const from = () => {
      const title = (it.from?.session && taskTitle(it.from.session)) || it.from?.title;
      if (!title || byTask()) return undefined;
      const there = !!(it.from?.session && taskTitle(it.from.session) !== undefined);
      return { label: title, title: t(there ? 'stuff.card.fromTitle' : 'stuff.card.fromGone'), onClick: there ? () => run(e, 'task') : undefined };
    };
    return (
      <StuffCard
        id={it.id}
        kind={it.kind}
        icon={look().icon}
        title={it.title || it.name}
        label={look().label}
        thumb={thumbOf(it)}
        facts={look().facts}
        time={look().time}
        version={look().version}
        pub={look().pub && { ...look().pub, label: look().pub?.label ?? '', ...(it.published && !it.publishing && { onClick: () => run(e, 'pubcopy') }) }}
        note={it.note}
        openData={look().openData}
        tags={it.tags}
        from={from()}
        byAgent={it.by === 'agent' ? t('stuff.card.byAgent') : undefined}
        pending={!!it.pending}
        starred={!!it.starred}
        held={held(it.id)}
        launch={(() => {
          const l = launchLook(it, t);
          return l && { ...l, onClick: () => run(e, 'open') };
        })()}
        copy={canCopy(it) ? { label: t(it.kind === 'link' ? 'stuff.card.copyLink' : 'stuff.card.copy'), onClick: () => run(e, 'copy') } : undefined}
        download={canDownload(it) ? { label: t('stuff.card.download'), onClick: () => run(e, 'download') } : undefined}
        more={{ label: t('stuff.card.more'), onClick: (b) => openMenu(e, b) }}
        star={{ label: t(it.starred ? 'stuff.card.unstarLabel' : 'stuff.card.starLabel'), title: t(it.starred ? 'stuff.card.unstar' : 'stuff.card.star'), onClick: () => run(e, 'star') }}
        onOpen={() => run(e, 'open')}
        onMenuAt={() => setMenuFor(e)}
        onKeyDown={(ev) => onCardKey(e, ev)}
      />
    );
  };

  const kinds = (): JSX.Element => (
    <For each={chips().chips}>
      {(ch) => <StuffKind label={ch.id === 'all' ? t('stuff.kind.all') : t(kindOf(ch.id).many)} count={ch.count} icon={ch.id === 'all' ? undefined : kindOf(ch.id).icon} on={chips().kind === ch.id} onPick={() => pick({ kind: ch.id })} />}
    </For>
  );

  return (
    <>
      <StuffFrame
        ref={(el) => (root = el)}
        title={t('stuff.title')}
        role={t('stuff.role')}
        count={headCount(items(), t, fmtSize)}
        actions={[
          { id: 'makePane', icon: 'app', label: t('stuff.make.title'), title: t('stuff.make.headTitle'), onClick: () => c.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'stuff.make' as never, params: {} } }) },
          { id: 'add', icon: 'plus', label: t('stuff.page.add'), title: t('stuff.page.addTitle'), onClick: () => picker?.click() },
        ]}
        close={stacked() ? undefined : { label: t('stuff.page.close'), onClick: () => c.navigate({ t: 'home' }) }}
        back={stacked() ? { label: t('stuff.page.back'), onClick: () => c.navigate({ t: 'back' }) } : undefined}
        search={{ value: q(), placeholder: t('stuff.page.search'), label: t('stuff.page.searchLabel'), onInput: setQ }}
        kinds={<Show when={chips().chips.length > 1}>{kinds()}</Show>}
        starred={items().some((x) => x.starred) || starred() ? { on: starred(), label: t('stuff.page.starred'), title: t('stuff.page.starredTitle'), onClick: () => setStarred(!starred()) } : undefined}
        sort={{ icon: byTask() ? 'agent' : 'clock', label: byTask() ? t('stuff.page.byTask') : t('stuff.page.byTime'), title: t(byTask() ? 'stuff.page.groupByTime' : 'stuff.page.groupByTask'), onClick: () => pick({ group: byTask() ? 'time' : 'task' }) }}
        view={{ list: list(), gridLabel: t('stuff.page.grid'), listLabel: t('stuff.page.list'), onPick: (l) => pick({ layout: l ? 'list' : 'grid' }) }}
        drop={{ title: t('stuff.drop.title'), sub: t('stuff.drop.sub'), onFiles: (f) => upload(f) }}
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
        <input
          ref={(el) => (picker = el)}
          type="file"
          multiple
          hidden
          onChange={(ev) => {
            upload(ev.currentTarget.files);
            ev.currentTarget.value = '';
          }}
        />
        <Show
          when={visible().length > 0}
          fallback={
            <StuffNone
              text={t('stuff.page.none', { what: noneWhat(q(), chips().kind, starred(), t) })}
              clear={t('stuff.page.showAll')}
              onClear={() => {
                setQ('');
                setStarred(false);
                pick({ kind: 'all' });
              }}
            />
          }
        >
          <ContextMenu part="stuff.menu" items={menuItems()} onSelect={(v) => menuFor() && run(menuFor() as Entry, v as ActionId)}>
            <For each={groups()}>
              {(g) => (
                <StuffGroup label={g.label} count={g.items.length} task={g.task ? { label: t('stuff.page.openTask'), onClick: () => run(entryOf(g.items[0]?.id ?? '') as Entry, 'task') } : undefined}>
                  <For each={g.items}>{(it) => (entryOf(it.id) ? card(entryOf(it.id) as Entry) : null)}</For>
                </StuffGroup>
              )}
            </For>
          </ContextMenu>
        </Show>
        <PaneCards client={c} />
      </StuffFrame>
      <Windows client={c} shown={shown()} close={() => setShown(undefined)} live={(k) => entries().find((x) => x.key === k)} run={run} publishTo={() => st.publishTo()[0]} />
      <EditDetails client={c} e={editing()?.e} anchor={editing()?.anchor} close={() => setEditing(undefined)} save={(e, f) => void c.run(updateItem, { key: e.key, id: e.it.id, title: f.title, note: f.note, tags: f.tags }).catch(() => {})} />
    </>
  );
}


export type { EntityKey };
