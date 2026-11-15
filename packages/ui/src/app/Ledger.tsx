// size-ok: one kit part whose rows, keys, search and scrubber share its signals (ADR-0068)
// The task history as a ledger (ADR-0068, docs/design/history-redesign.md § A): each earlier turn one line led by
// what came of it, the user's words second; wisp's notices faint, a run of them one line; today and yesterday open,
// earlier days a card with an outcome strip; search and filters; a scrubber of one tick per turn down the right edge;
// one turn open at a time, in place (its head row stays where it was). Keys, while focus is in the ledger: j/k step,
// / searches, [ ] jump between failures, Esc closes. The model is ./ledger-model.ts (plain, tested on its own).
//
// It loads with the task page (its own sheet, ADR-0011/0067), never on the first screen.
import { For, Index, Show, createEffect, createMemo, createSignal, onCleanup, untrack, type Accessor, type JSX } from 'solid-js';
import { flag, styleOf, sub, type KitProps, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { Each, VirtualList } from '../layout/VirtualList.tsx';
import {
  FILTERS,
  SCRUB_ABOVE,
  VIRTUAL_ABOVE,
  counts,
  dayKind,
  dayOf,
  daysOf,
  firstLine,
  ledgerKey,
  ledgerRows,
  matches,
  nextFailure,
  searching,
  stepFrom,
  ticks,
  titleOf,
  turnAt,
  type DayKind,
  type LedgerFilter,
  type LedgerRow,
  type LedgerTurn,
  type Outcome,
} from './ledger-model.ts';
import '../css/ledger.css';

/** The ledger's words, from the shell's catalogue (the kit shows no copy of its own). */
export type LedgerString =
  | 'search'
  | `filter.${LedgerFilter}`
  | 'count'
  | 'today'
  | 'yesterday'
  | 'turns'
  | 'matches'
  | 'span'
  | 'sum'
  | 'fold'
  | 'show'
  | 'notices'
  | 'you'
  | 'wisp'
  | 'none'
  | 'empty'
  | 'earlier'
  | 'later'
  | 'scrub'
  | 'at'
  | `glyph.${Outcome | 'notice'}`;

export interface LedgerProps extends KitProps {
  /** the earlier turns, oldest first (the latest ones, `tail`, are drawn whole after them) */
  readonly turns: readonly LedgerTurn[];
  /** every turn of the task, the tail's too (the count, and whether the scrubber shows) */
  readonly total: number;
  readonly now: number;
  /** the one turn open in place */
  readonly open: string | undefined;
  readonly onOpen: (id: string | undefined) => void;
  /** the open turn, whole: its prompt, hero, answer, files and actions */
  readonly body: (id: string) => JSX.Element;
  /** after the earlier turns: the latest turn (the live one) and what follows it */
  readonly tail: JSX.Element;
  readonly t: (key: LedgerString, vars?: Readonly<Record<string, string | number>>) => string;
  /** an earlier day's name ("Mon 9 Nov") */
  readonly dayName: (day: number) => string;
  /** a turn's time of day ("15:37") */
  readonly clock: (at: number) => string;
  /** search and filters (a short task goes without) */
  readonly tools?: boolean;
}

const GLYPH: Readonly<Record<Outcome, IconName>> = { done: 'done', failed: 'warn', answer: 'info', running: 'clock' };

/** The nearest box that scrolls `el` (the task's body, or the virtual list). */
function scrollerOf(el: HTMLElement): HTMLElement | undefined {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const o = getComputedStyle(p).overflowY;
    if (o === 'auto' || o === 'scroll') return p;
  }
  return undefined;
}

const TYPING = 'input, textarea, select, [contenteditable="true"], [contenteditable=""]';

export function Ledger(props: LedgerProps): JSX.Element {
  const part = (): PartName => props.part ?? 'task.ledger';
  const t = props.t;
  const [q, setQ] = createSignal('');
  const [filter, setFilter] = createSignal<LedgerFilter>('all');
  const [days, setDays] = createSignal<ReadonlyMap<number, boolean>>(new Map());
  const [unfolded, setUnfolded] = createSignal<ReadonlySet<string>>(new Set());
  const view = () => ({ q: q(), filter: filter(), days: days(), unfolded: unfolded(), open: props.open });
  const rows = createMemo(() => ledgerRows(props.turns, props.now, view()));
  const found = createMemo(() => (searching(view()) ? props.turns.filter((x) => matches(x, q(), filter())) : props.turns));
  const n = createMemo(() => counts(props.turns));
  const dayCount = createMemo(() => daysOf(props.turns, props.now).length);
  const scrub = (): boolean => props.total > SCRUB_ABOVE && props.turns.length > 0;
  // from the task's size, not the lines shown: opening a day or typing a search never swaps the list (and remounts it)
  const virtual = (): boolean => props.turns.length > VIRTUAL_ABOVE;
  let root!: HTMLDivElement;
  let search: HTMLInputElement | undefined;
  let vlist: HTMLDivElement | undefined;

  // the scrubber is as tall as what shows of the page (it stays in view as the page scrolls)
  const [viewH, setViewH] = createSignal(0);
  createEffect(() => {
    if (!scrub() || typeof ResizeObserver === 'undefined') return;
    const box = scrollerOf(root);
    if (!box) return;
    const ro = new ResizeObserver(() => setViewH(box.clientHeight));
    ro.observe(box);
    onCleanup(() => ro.disconnect());
  });

  const rowEl = (id: string): HTMLElement | null => root.querySelector<HTMLElement>(`[data-ledger-turn="${CSS.escape(id)}"]`);

  /** Brings a turn's head row into view (after it rendered); a virtual list scrolls to its place first. */
  const reveal = (id: string, block: ScrollLogicalPosition): void => {
    const go = (): boolean => {
      const el = rowEl(id);
      // never smooth: nothing moves on its own for someone who asked for less motion, and a jump is instant for all
      el?.scrollIntoView({ block, behavior: 'instant' });
      el?.focus({ preventScroll: true });
      return !!el;
    };
    queueMicrotask(() => {
      if (go() || !virtual() || !vlist) return;
      const i = rows().findIndex((r) => r.k === 'turn' && r.turn.id === id);
      if (i < 0) return;
      vlist.scrollTop = (i / rows().length) * vlist.scrollHeight;
      requestAnimationFrame(() => void go());
    });
  };

  /** Opens a turn (or closes it). `from`: the row pressed, which stays where it is on screen while the turn that was
   * open above it closes. */
  const openTurn = (id: string | undefined, from?: HTMLElement): void => {
    const box = from ? scrollerOf(from) : undefined;
    let before = from?.getBoundingClientRect().top ?? 0;
    const hold = (): void => {
      if (!from?.isConnected || !box) return;
      const now = from.getBoundingClientRect().top;
      box.scrollTop += now - before;
      before = now;
    };
    props.onOpen(id);
    hold();
    queueMicrotask(hold);
  };

  /** Jumps to a turn from outside its row (the scrubber, a key): the list shows it whole, its day open. */
  const jump = (x: LedgerTurn, block: ScrollLogicalPosition, clear: boolean): void => {
    if (clear && searching(view()) && !matches(x, q(), filter())) {
      setQ('');
      setFilter('all');
    }
    props.onOpen(x.id);
    reveal(x.id, block);
  };

  const onKey = (e: KeyboardEvent): void => {
    if (e.defaultPrevented || e.isComposing) return;
    const k = ledgerKey(e);
    if (!k) return;
    const target = e.target as HTMLElement | null;
    if (target === search) {
      if (k !== 'close') return;
      e.preventDefault();
      if (q()) setQ('');
      else search.blur();
      return;
    }
    if (target?.closest(TYPING) || target?.closest('[data-kit="ledger-tail"]')) return;
    const list = found();
    let next: LedgerTurn | undefined;
    if (k === 'search') {
      if (!props.tools || !search) return;
      e.preventDefault();
      search.focus();
      search.select();
      return;
    }
    if (k === 'close') {
      if (props.open === undefined) return; // nothing open: Escape goes on (Back)
      e.preventDefault();
      const id = props.open;
      props.onOpen(undefined);
      queueMicrotask(() => rowEl(id)?.focus({ preventScroll: true }));
      return;
    }
    if (k === 'next' || k === 'prev') next = stepFrom(list, props.open, k === 'next' ? 1 : -1);
    else next = nextFailure(list, props.open, k === 'nextFail' ? 1 : -1);
    e.preventDefault(); // handled, even at either end: the page's own j/k and [ ] don't also run
    if (next) jump(next, 'nearest', false);
  };

  const dayLabel = (day: number, kind: DayKind): string => (kind === 'today' ? t('today') : kind === 'yesterday' ? t('yesterday') : props.dayName(day));
  const toggleDay = (day: number, open: boolean): void => void setDays(new Map([...days(), [day, open]]));

  const glyph = (x: LedgerTurn): JSX.Element => (
    <span data-kit="ledger-glyph" data-tone={x.notice !== undefined && x.outcome !== 'failed' ? 'notice' : x.outcome}>
      <Icon name={x.notice !== undefined && x.outcome !== 'failed' ? 'spark' : GLYPH[x.outcome]} size="sm" label={t(x.notice !== undefined && x.outcome !== 'failed' ? 'glyph.notice' : `glyph.${x.outcome}`)} />
    </span>
  );

  /** A turn's one line (its head row once open): the glyph, the outcome, the time; the prompt under it. */
  const head = (x: Accessor<LedgerTurn>, open: Accessor<boolean>): JSX.Element => (
    <button
      type="button"
      data-kit="ledger-row"
      data-part={sub(part(), 'row')}
      data-ledger-turn={x().id}
      data-turn={open() ? undefined : x().id}
      data-notice={flag(x().notice !== undefined)}
      data-open={flag(open())}
      aria-expanded={open() ? 'true' : 'false'}
      onClick={(e) => {
        e.currentTarget.focus({ preventScroll: true });
        openTurn(open() ? undefined : x().id, e.currentTarget);
      }}
    >
      {glyph(x())}
      <Show
        when={x().notice === undefined}
        fallback={
          <span data-kit="ledger-hero">
            {/* "wisp · CI finished"; a notice that already names wisp isn't said twice */}
            <b>{((k) => (k.toLowerCase().startsWith(t('wisp').toLowerCase()) ? k : `${t('wisp')} · ${k}`))(x().notice ?? '')}</b>
            <Show when={x().hero}>{(h) => ` → ${h()}`}</Show>
          </span>
        }
      >
        <span data-kit="ledger-hero">{titleOf(x())}</span>
      </Show>
      <span data-kit="ledger-time">{props.clock(x().at)}</span>
      <Show when={x().notice === undefined && x().hero}>
        <span data-kit="ledger-ask">{`${t('you')}${firstLine(x().prompt)}`}</span>
      </Show>
    </button>
  );

  // the head row is made once and never swapped: it keeps its place on screen and its focus as the turn opens and closes
  const turnRow = (x: Accessor<LedgerTurn>): JSX.Element => {
    const open = (): boolean => props.open === x().id;
    return (
      <div data-kit="ledger-turn" data-part={sub(part(), 'open')} data-open={flag(open())}>
        {head(x, open)}
        <Show when={open()}>
          {/* its key holds its id: the view is made once per opening, never again as the row it came from changes */}
          <div data-kit="ledger-body">{((id) => untrack(() => props.body(id)))(x().id)}</div>
          <div data-kit="ledger-step">
            <button type="button" data-kit="ledger-btn" data-part={sub(part(), 'earlier')} disabled={!stepFrom(found(), x().id, -1)} onClick={() => ((p) => p && jump(p, 'nearest', false))(stepFrom(found(), x().id, -1))}>
              {t('earlier')}
            </button>
            <button type="button" data-kit="ledger-btn" data-part={sub(part(), 'later')} disabled={!stepFrom(found(), x().id, 1)} onClick={() => ((p) => p && jump(p, 'nearest', false))(stepFrom(found(), x().id, 1))}>
              {t('later')}
            </button>
          </div>
        </Show>
      </div>
    );
  };

  type Of<K extends LedgerRow['k']> = Extract<LedgerRow, { k: K }>;
  const line = (r: Accessor<LedgerRow>): JSX.Element => {
    // a row's kind is in its key, so it never changes under it; what it says does
    switch (r().k) {
      case 'turn':
        return turnRow(() => (r() as Of<'turn'>).turn);
      case 'fold': {
        const v = (): Of<'fold'> => r() as Of<'fold'>;
        return (
          <button type="button" data-kit="ledger-fold" data-part={sub(part(), 'fold')} onClick={() => setUnfolded(new Set([...unfolded(), v().turns[0]?.id ?? '']))}>
            <Icon name="spark" size="sm" />
            <span data-kit="ledger-fold-text">{t('notices', { n: v().turns.length, kinds: v().kinds.join(', ') })}</span>
          </button>
        );
      }
      case 'card': {
        const v = (): Of<'card'> => r() as Of<'card'>;
        return (
          <button type="button" data-kit="ledger-card" data-part={sub(part(), 'day')} aria-label={t('show', { day: dayLabel(v().day, v().kind) })} onClick={() => toggleDay(v().day, true)}>
            <span data-kit="ledger-card-day">{dayLabel(v().day, v().kind)}</span>
            <span data-kit="ledger-card-span">{`${t('turns', { n: v().stats.count })} · ${t('span', { from: props.clock(v().stats.from), to: props.clock(v().stats.to) })}`}</span>
            <span data-kit="ledger-card-sum" data-failed={flag(v().stats.failed > 0)}>
              {t('sum', { done: v().stats.done, failed: v().stats.failed, files: v().stats.files })}
            </span>
            <span data-kit="ledger-strip" aria-hidden="true">
              <For each={v().strip}>{(tone) => <i data-tone={tone} />}</For>
            </span>
          </button>
        );
      }
      case 'day': {
        const v = (): Of<'day'> => r() as Of<'day'>;
        return (
          <div data-kit="ledger-day" data-part={sub(part(), 'dayhead')}>
            <span data-kit="ledger-day-name">{dayLabel(v().day, v().kind)}</span>
            <span data-kit="ledger-day-sub">{v().search ? t('matches', { n: v().count }) : t('turns', { n: v().count })}</span>
            <Show when={!v().search}>
              <button type="button" data-kit="ledger-btn" data-part={sub(part(), 'fold-day')} onClick={() => toggleDay(v().day, false)}>
                {t('fold')}
              </button>
            </Show>
          </div>
        );
      }
    }
  };

  // the scrubber: hovering previews a turn, a press (or a drag's end) opens it
  const [hover, setHover] = createSignal<{ readonly turn: LedgerTurn; readonly y: number } | undefined>();
  const [dragging, setDragging] = createSignal(false);
  let pressed = false;
  let track: HTMLDivElement | undefined;
  const under = (e: PointerEvent): { turn: LedgerTurn; y: number } | undefined => {
    if (!track) return undefined;
    const r = track.getBoundingClientRect();
    const turn = turnAt(props.turns, e.clientY - r.top, r.height);
    return turn && { turn, y: Math.min(Math.max(e.clientY - r.top, 0), r.height) };
  };
  const scrubKey = (e: KeyboardEvent): void => {
    const dir = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : 0;
    if (!dir) return;
    e.preventDefault();
    const next = stepFrom(props.turns, props.open, dir);
    if (next) jump(next, 'center', true);
  };
  const tickList = createMemo(() => ticks(props.turns, props.open));
  const openN = (): LedgerTurn | undefined => props.turns.find((x) => x.id === props.open);

  return (
    <div ref={root} data-kit="ledger" data-part={part()} data-scrub={flag(scrub())} onKeyDown={onKey} style={styleOf({ '--k-ledger-h': viewH() ? `${viewH()}px` : undefined })}>
      <Show when={props.tools && props.turns.length > 0}>
        <div data-kit="ledger-tools" data-part={sub(part(), 'tools')}>
          <label data-kit="ledger-search">
            <Icon name="search" size="sm" />
            <input
              ref={search}
              type="search"
              data-part={sub(part(), 'search')}
              value={q()}
              placeholder={t('search', { n: props.turns.length })}
              aria-label={t('search', { n: props.turns.length })}
              onInput={(e) => setQ(e.currentTarget.value)}
            />
            <kbd data-kit="ledger-kbd">/</kbd>
          </label>
          <div data-kit="ledger-chips" role="group">
            <For each={FILTERS}>
              {(f) => (
                <button type="button" data-kit="ledger-chip" data-part={sub(part(), 'filter')} aria-pressed={filter() === f ? 'true' : 'false'} onClick={() => setFilter(filter() === f && f !== 'all' ? 'all' : f)}>
                  {t(`filter.${f}`)}
                  <span data-kit="ledger-chip-n">{n()[f]}</span>
                </button>
              )}
            </For>
          </div>
          <span data-kit="ledger-count">{t('count', { n: props.total, d: dayCount() })}</span>
        </div>
      </Show>
      <div data-kit="ledger-wrap">
        <div data-kit="ledger-list" role="list" aria-label={t('scrub')}>
          <Show
            when={virtual()}
            fallback={
              <Each items={rows()} key={(r) => r.key}>
                {(r) => line(r)}
              </Each>
            }
          >
            <VirtualList items={rows()} key={(r) => r.key} estimate={44} part={sub(part(), 'virtual')} ref={(el) => (vlist = el)}>
              {(r) => line(r)}
            </VirtualList>
          </Show>
          <Show when={searching(view()) && rows().length === 0}>
            <p data-kit="ledger-none">{q().trim() ? t('none', { q: q().trim() }) : t('empty')}</p>
          </Show>
          <div data-kit="ledger-tail">{props.tail}</div>
        </div>
        <Show when={scrub()}>
          <div
            data-kit="ledger-scrub"
            data-part={sub(part(), 'scrubber')}
            data-dragging={flag(dragging())}
            role="slider"
            tabIndex={0}
            aria-label={t('scrub')}
            aria-orientation="vertical"
            aria-valuemin={1}
            aria-valuemax={props.turns.length}
            aria-valuenow={openN()?.n ?? props.turns.length}
            aria-valuetext={openN() ? titleOf(openN() as LedgerTurn) : undefined}
            onKeyDown={scrubKey}
            onPointerMove={(e) => setHover(under(e))}
            onPointerLeave={() => !dragging() && setHover(undefined)}
            onPointerDown={(e) => {
              if (e.button !== 0) return;
              e.currentTarget.setPointerCapture(e.pointerId);
              pressed = true;
              setDragging(true);
              setHover(under(e));
            }}
            onPointerUp={(e) => {
              const at = pressed ? under(e) : undefined;
              pressed = false;
              setDragging(false);
              if (e.pointerType !== 'mouse') setHover(undefined);
              if (at) jump(at.turn, 'center', true);
            }}
            onPointerCancel={() => {
              pressed = false;
              setDragging(false);
              setHover(undefined);
            }}
          >
            <div ref={track} data-kit="ledger-ticks">
              <Index each={tickList()}>{(k) => <i data-tone={k().tone} style={styleOf({ '--k-at': `${(k().at * 100).toFixed(3)}%` })} />}</Index>
            </div>
            <Show when={hover()}>
              {(h) => (
                <div data-kit="ledger-bubble" data-part={sub(part(), 'preview')} style={styleOf({ '--k-y': `${h().y}px` })}>
                  <span data-kit="ledger-bubble-at">
                    {t('at', { day: dayLabel(dayOf(h().turn.at), dayKind(dayOf(h().turn.at), props.now)), time: props.clock(h().turn.at), n: h().turn.n, total: props.total })}
                  </span>
                  <span data-kit="ledger-bubble-hero">
                    {glyph(h().turn)}
                    <span>{titleOf(h().turn)}</span>
                  </span>
                </div>
              )}
            </Show>
          </div>
        </Show>
      </div>
    </div>
  );
}
