// ListRow: one row of a list (a task, a machine, a file). Every cell is always rendered in a fixed
// column, so selecting, opening or hovering a row moves nothing in it: the time sits in a
// fixed-width column and hover-only actions appear beside it, never on top of it (v1 #188, #209).
// In the phone apps (ADR-0065 build step 4, design/mobile.md §A.2, §A.7) the same row is an M3 list item
// (1/2/3 lines: 56/72/88 dp, a 40 dp leading element, trailing supporting text) or an iOS table cell
// (44 pt min, inset separators, a chevron where it pushes). The row says how many lines it has
// (`data-row-lines`) and whether it leads with something (`data-lead`) on a phone only, so the browser's
// HTML stays byte for byte v1's (test/goldens/platform-browser-rows.json).
import { Show, createEffect, createMemo, createSignal, onCleanup, onMount, type JSX } from 'solid-js';
import { secondLineEmpty } from './second-line.ts';
import { watchTimeWidth } from './time-width.ts';
import { flag, sub, type KitProps, type PartName } from '../core/props.ts';
import type { ReorderAttrs } from '../core/reorder.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';

export interface ListRowProps extends KitProps {
  readonly title: string;
  /** A second line: where it lives, what it's doing. */
  readonly sub?: JSX.Element;
  /** An icon, avatar or status mark before the text. */
  readonly lead?: JSX.Element;
  /** feed layout: a last line across the whole row, under the sub and the pills (v1 .it-run, the run line) */
  readonly foot?: JSX.Element;
  /** Pills after the text, in their own column. */
  readonly pills?: JSX.Element;
  /** Usually a <Time>; its column is fixed-width. */
  readonly time?: JSX.Element;
  /** Buttons beside the time: on hover and focus on desktop, always on touch. */
  readonly actions?: JSX.Element;
  /** The row is picked (selection, keyboard cursor). */
  readonly selected?: boolean;
  /** The row is what's open (aria-current). */
  readonly current?: boolean;
  /** A quieter title, for a finished row that must still read (ink-2, design/ui.md). */
  readonly quiet?: boolean;
  /** The title in ink and 600 (v1 .item.unread / .st-waiting / .open .it-title): it wants you. Additive (2026-11-12). */
  readonly strong?: boolean;
  /** Opening the row. Without it (and without `href`) the row is static text. */
  readonly onOpen?: (e: MouseEvent) => void;
  /** Opening as a link, so middle-click and Copy link work. */
  readonly href?: string;
  readonly onContextMenu?: (e: MouseEvent) => void;
  /** `feed`: v1's task row (#feed .it-head): the time and the actions on the title's line, the pills on the
   *  line under it, beside the sub, so the title keeps the row's width. Default `line`: everything on one line. */
  readonly layout?: 'line' | 'feed';
  /** Dragging rows into your own order: `reorderable(list, id, ids, onReorder)` (v1 #772, the Projects list). */
  readonly reorder?: ReorderAttrs;
  /** More part names for the row's elements, so a list keeps another contract's names too (v1's `card`,
   *  `card.title`… that `qa/parity.mjs` measures): `{ row: 'card', title: 'card.title' }`. Additive (2026-11-12). */
  readonly also?: Partial<Readonly<Record<RowPiece, string>>> | undefined;
  /** the sub line's tone (feed: v1 .q-live running in accent ink, waiting in warn) and its tooltip. Additive (2026-11-12). */
  readonly subTone?: 'accent-ink' | 'warn' | 'muted' | undefined;
  readonly subTitle?: string | undefined;
  /** feed: how many quick actions the row has (0–2): with a mouse the title makes room for them while they show; on a
   *  touch screen (hover: none) they leave the row, so the title keeps its own edge. Additive (2026-11-12). */
  readonly quick?: 0 | 1 | 2 | undefined;
  /** Opening the row pushes a page (a setting, a computer): iOS draws the disclosure chevron at its end
   *  (HIG); Android and the browser draw nothing. A task row opens beside the list, so it has none (Mail).
   *  Additive (ADR-0065 step 4). */
  readonly push?: boolean | undefined;
  /** feed, the phone apps' selection mode (ADR-0065 step 13): set (true or false) while the list picks rows; the row
   *  leads with its check (Android's filled circle, iOS's edit-mode checkmark) in place of `lead`, a tap toggles it
   *  (`onOpen`), and it says so (aria-pressed). Unset, nothing of it is drawn. Additive (2026-11-15). */
  readonly picked?: boolean | undefined;
  /** feed (docs/ux/2026-11-18-desktop.md #2): the quick actions (RowButton `slot` 1–3) take the time's fixed slot on
   *  hover and keyboard focus, by opacity, so the title never moves. Additive (2026-11-18). */
  readonly slots?: boolean | undefined;
}

/** How many lines the row's text takes on a phone (M3 one/two/three-line list items; iOS cell heights). */
export const rowLines = (row: Pick<ListRowProps, 'sub' | 'foot'>): '1' | '2' | '3' => (row.foot ? '3' : row.sub !== undefined && row.sub !== null && row.sub !== false ? '2' : '1');

/** A row's element props, each built once. The row's attributes test whether a cell is there (data-time, data-foot,
 *  data-lead, data-row-lines) and all of them are one render effect, rerun when any changes (a tap moves `current`);
 *  reading the prop there built the cell again each time just to test it, and once more to show it (F-phone-8). The
 *  memo builds it once, and the attributes and the cell share it. */
function cells(props: ListRowProps): { readonly lead: () => JSX.Element; readonly sub: () => JSX.Element; readonly foot: () => JSX.Element; readonly time: () => JSX.Element } {
  return { lead: createMemo(() => props.lead), sub: createMemo(() => props.sub), foot: createMemo(() => props.foot), time: createMemo(() => props.time) };
}

type RowPiece = 'row' | 'lead' | 'main' | 'title' | 'sub' | 'foot' | 'pills' | 'time' | 'actions';

/** The row's one click target: a button or link stretched over the row by CSS (::after). */
/** `list.row.title`, and the name `also` gives it. */
const named = (row: ListRowProps, part: PartName, piece: RowPiece): string => {
  const own = piece === 'row' ? part : sub(part, piece);
  const more = row.also?.[piece];
  return more ? `${own} ${more}` : own;
};

function Hit(props: { readonly row: ListRowProps; readonly part: PartName }): JSX.Element {
  const title = (): JSX.Element => (
    <span data-kit="list-row-title" data-text="control" data-part={named(props.row, props.part, 'title')} title={props.row.title}>
      {props.row.title}
    </span>
  );
  const current = (): 'page' | undefined => (props.row.current ? 'page' : undefined);
  const plain = (): JSX.Element => (
    <Show when={props.row.onOpen} fallback={<span data-kit="list-row-hit">{title()}</span>}>
      {(open) => (
        <button type="button" data-kit="list-row-hit" aria-current={current()} onClick={(e) => open()(e)}>
          {title()}
        </button>
      )}
    </Show>
  );
  return (
    <Show when={props.row.href} fallback={plain()}>
      {(href) => (
        <a href={href()} data-kit="list-row-hit" aria-current={current()} onClick={(e) => props.row.onOpen?.(e)}>
          {title()}
        </a>
      )}
    </Show>
  );
}

export { feedDoneColumn } from './feed-done.ts';

/** A row's quick action (v1 .it-quick, .it-later): an icon the pointer presses, hidden from assistive tech and the
 *  Tab order as v1's are (the row's head is the one button in it; the keys E and L do the same from the keyboard).
 *  It never opens the row. v1's element tree: the box, the glyph, its path. */
export function RowButton(props: {
  readonly icon: IconName;
  readonly label: string;
  readonly tone?: 'good' | 'bad';
  readonly pressed?: boolean;
  readonly part?: PartName;
  /** its place in the time's slot (ListRow `slots`: 1 Later, 2 Done, 3 ⋯): a button with its name and tooltip, out of
   *  the Tab order and the accessibility tree (the row is the one stop; its keys do the same). Additive (2026-11-18). */
  readonly slot?: 1 | 2 | 3;
  readonly onClick: (el: HTMLElement) => void;
}): JSX.Element {
  if (props.slot !== undefined)
    return (
      // out of the accessibility tree as the span was (the row is the one button; nested ones confuse readers): the
      // keys E and L and the row's menu (its context-menu key) do the same; the name stays for the tooltip
      <button
        type="button"
        tabIndex={-1}
        aria-hidden="true"
        aria-label={props.label}
        title={props.label}
        data-kit="icon-button"
        data-part={props.part ?? 'ui.row-button'}
        data-variant="ghost"
        data-size="sm"
        data-slot={props.slot}
        data-tone={props.tone}
        data-on={flag(props.pressed)}
        aria-pressed={props.pressed === undefined ? undefined : props.pressed}
        onClick={(e) => {
          e.stopPropagation();
          props.onClick(e.currentTarget);
        }}
      >
        <Icon name={props.icon} size="md" />
      </button>
    );
  return (
    <span
      aria-hidden="true"
      title={props.label}
      data-kit="icon-button"
      data-part={props.part ?? 'ui.row-button'}
      data-variant="ghost"
      data-size="sm"
      data-tone={props.tone}
      data-on={flag(props.pressed)}
      onClick={(e) => {
        e.stopPropagation();
        props.onClick(e.currentTarget);
      }}
    >
      <Icon name={props.icon} size="md" />
    </span>
  );
}

/** v1 feed.js runLine(): the harness's logo and name (and its model), then where it runs after a `·`. */
export function RowRun(props: {
  readonly logo?: JSX.Element;
  readonly name: string;
  /** v1 .ir-m: the model in its own span, so a long one ellipsises before the place does */
  readonly model?: string | undefined;
  /** v1 .it-run[title]: harness · model · effort · place, on the whole line */
  readonly title?: string | undefined;
  readonly where: { readonly icon: IconName; readonly text: string };
  /** it runs on this computer: the phone looks leave the place out (it says nothing a row from elsewhere wouldn't; UX
   *  2026-11-18 phone #10). Additive. */
  readonly local?: boolean | undefined;
  /** the browser's two-line rows (docs/ux/2026-11-18-desktop.md #2): no place at all (the host passes it only for a
   *  task on this computer). Additive (2026-11-18). */
  readonly noWhere?: boolean | undefined;
  readonly part?: PartName;
}): JSX.Element {
  const look = usePlatform();
  return (
    // the browser's line stays as it was: only a phone look marks it
    <div data-kit="row-run" data-part={props.part ?? 'ui.row-run'} title={props.title} data-local={flag(look() !== 'web' && props.local === true)}>
      {props.logo}
      <span data-kit="ir-h">{props.name}</span>
      {/* a model shorter than the 7ch floor keeps its own width (TRY-12: "opus" had a gap before the next dot) */}
      <Show when={props.model}>{(m) => <span data-kit="ir-m" data-long={m().length > 5 ? '' : undefined}>{m()}</span>}</Show>
      <Show when={!props.noWhere}>
        <span data-kit="ir-w">
          <Icon name={props.where.icon} size="xs" />
          <span>{props.where.text}</span>
        </span>
      </Show>
    </div>
  );
}

/** v1 #feed .it-head, element for element (qa/parity.mjs compares a hovered row's tree by index): the head is the
 *  row's one button; in it the dot, the title, the sub's words, the run line, the pills with the time last, then the
 *  quick actions. */
function FeedRow(props: ListRowProps & { readonly partName: PartName }): JSX.Element {
  const p = (): PartName => props.partName;
  const open = (e: MouseEvent): void => props.onOpen?.(e);
  const phone = ((look) => (): boolean => look() !== 'web')(usePlatform());
  const c = cells(props);
  let row: HTMLDivElement | undefined;
  let pills: HTMLDivElement | undefined;
  let subEl: HTMLDivElement | undefined;
  // nothing to see on the second line (no summary; the pills' cell holds only the time, or a plugin's pill that drew
  // nothing): the row is its title and its run line, no blank line between (v1; QA round 3 #6). Read from what the row
  // drew, as a plugin's pills come and go; only the rows on screen exist (VirtualList), each watching its two cells.
  const [oneLine, setOneLine] = createSignal(false);
  onMount(() => {
    if (phone() || !subEl || !pills || typeof MutationObserver !== 'function') return;
    const s = subEl;
    const pl = pills;
    const read = (): void => void setOneLine(secondLineEmpty(s, pl));
    read();
    const mo = new MutationObserver(read);
    mo.observe(s, { childList: true, subtree: true, characterData: true });
    mo.observe(pl, { childList: true, subtree: true, characterData: true });
    onCleanup(() => mo.disconnect());
  });
  // with a mouse the quick actions sit before the time, whatever its width (B30); the time is the pills' last
  // watched per element, not per label: "12m" becoming "13m" re-ran this and unwatched/rewatched the same time (#13)
  const timeEl = createMemo(() => (c.time() === undefined || !props.quick || phone() || !pills ? undefined : (pills.lastElementChild ?? undefined)));
  createEffect(() => {
    const el = timeEl();
    if (el && row) onCleanup(watchTimeWidth(row, el));
  });
  return (
    <div
      ref={row}
      {...props.reorder}
      id={props.id}
      role="button"
      tabIndex={0}
      aria-current={props.current ? 'page' : undefined}
      data-kit="list-row"
      data-part={named(props, p(), 'row')}
      data-selected={flag(props.selected)}
      data-current={flag(props.current)}
      data-quiet={flag(props.quiet)}
      data-strong={flag(props.strong)}
      data-time={flag(c.time() !== undefined)}
      data-layout="feed"
      data-quick={props.quick ? String(props.quick) : undefined}
      data-slots={flag(props.slots)}
      data-foot={flag(!!c.foot())}
      data-one-line={flag(oneLine())}
      data-interactive={flag(props.onOpen !== undefined)}
      data-row-lines={phone() ? rowLines({ sub: c.sub(), foot: c.foot() }) : undefined}
      data-lead={flag(phone() && c.lead() !== undefined)}
      data-picking={flag(props.picked !== undefined)}
      data-picked={flag(props.picked)}
      aria-pressed={props.picked}
      onClick={open}
      onKeyDown={(e) => {
        if (e.target !== e.currentTarget || (e.key !== 'Enter' && e.key !== ' ')) return;
        e.preventDefault();
        open(new MouseEvent('click'));
      }}
      onContextMenu={(e) => props.onContextMenu?.(e)}
    >
      <Show when={props.picked !== undefined} fallback={c.lead()}>
        <span data-kit="row-pick" data-part={sub(p(), 'pick')} aria-hidden="true">
          <Show when={props.picked}>
            <Icon name="check" size="sm" />
          </Show>
        </span>
      </Show>
      <span data-kit="list-row-title" data-text="control" data-part={named(props, p(), 'title')} title={props.title}>
        {props.title}
      </span>
      <div ref={subEl} data-kit="list-row-sub" data-part={named(props, p(), 'sub')} data-tone={props.subTone} title={props.subTitle}>
        {c.sub()}
      </div>
      {c.foot()}
      <div ref={pills} data-kit="list-row-pills" data-part={named(props, p(), 'pills')}>
        {props.pills}
        <Show when={!phone()}>{c.time()}</Show>
      </div>
      {/* on a phone the time is the row's own trailing cell: its line over the pills' line, never on them (r285) */}
      <Show when={phone()}>{c.time()}</Show>
      {props.actions}
    </div>
  );
}

export function ListRow(props: ListRowProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.list-row';
  if (props.layout === 'feed') return <FeedRow {...props} partName={part()} />;
  const look = usePlatform();
  const phone = (): boolean => look() !== 'web';
  const c = cells(props);
  return (
    <div
      {...props.reorder}
      id={props.id}
      data-kit="list-row"
      data-part={named(props, part(), 'row')}
      aria-selected={props.selected === undefined ? undefined : props.selected}
      data-selected={flag(props.selected)}
      data-current={flag(props.current)}
      data-quiet={flag(props.quiet)}
      data-strong={flag(props.strong)}
      data-time={flag(c.time() !== undefined)}
      data-foot={flag(!!c.foot())}
      data-interactive={flag(props.onOpen !== undefined || props.href !== undefined)}
      data-row-lines={phone() ? rowLines({ sub: c.sub(), foot: c.foot() }) : undefined}
      data-lead={flag(phone() && c.lead() !== undefined)}
      onContextMenu={(e) => props.onContextMenu?.(e)}
    >
      <div data-kit="list-row-lead" data-part={named(props, part(), 'lead')}>
        {c.lead()}
      </div>
      <div data-kit="list-row-main" data-part={named(props, part(), 'main')}>
        <Hit row={props} part={part()} />
        <div data-kit="list-row-sub" data-part={named(props, part(), 'sub')}>
          {c.sub()}
        </div>
      </div>
      <Show when={c.foot()}>
        <div data-kit="list-row-foot" data-part={named(props, part(), 'foot')}>
          {c.foot()}
        </div>
      </Show>
      <div data-kit="list-row-pills" data-part={named(props, part(), 'pills')}>
        {props.pills}
      </div>
      <div data-kit="list-row-time" data-part={named(props, part(), 'time')}>
        {c.time()}
      </div>
      <div data-kit="list-row-actions" data-part={named(props, part(), 'actions')}>
        {props.actions}
      </div>
      <Show when={look() === 'ios' && props.push === true && (props.onOpen !== undefined || props.href !== undefined)}>
        <span data-kit="list-row-chevron" data-part={sub(part(), 'chevron')} aria-hidden="true">
          <Icon name="chevron" size="sm" />
        </span>
      </Show>
    </div>
  );
}
