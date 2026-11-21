// The follow-ups queued for after the run, inside the reply box over its text (v1 queued.js, composer.css `.c-queued`,
// `.cq`): a dashed row each, what it waits for (Next · after Claude Code finishes, Held after Stop, Queued #2) in the
// accent, then what it says on one line. On hover (or focus) its actions slide over the end of the text on the row's
// fill (Send now, Combine with the next one, the pen) so nothing in the row moves; × is always there. Two or more: a
// grip in the left padding (drag it, or ↑/↓ on it). A right-click is the row's menu, and on a touch screen a tap is.
// A row being edited keeps its tag and ×, its text becomes the caller's box. Its CSS loads with it (queued.css).
import { Show, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName, type WithChildren } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import '../css/queued.css';

export interface QueuedListProps extends KitProps, WithChildren {
  readonly label: string;
  /** the list element, for a gesture that binds to it (the phone's long-press drag) */
  readonly ref?: (el: HTMLElement) => void;
}

/** The column of rows, 4px apart (v1 `.c-queued`). Its children are QueuedRow. */
export function QueuedList(props: QueuedListProps): JSX.Element {
  return (
    <div
      ref={(el) => props.ref?.(el)}
      data-kit="cq-list"
      data-part={props.part ?? 'ui.queued'}
      role="list"
      aria-label={props.label}
      onContextMenu={(e) => {
        // between the rows: no row's menu (a menu around the list would open the last one asked for)
        if (!(e.target instanceof Element && e.target.closest('[data-kit="cq-row"]'))) e.stopPropagation();
      }}
    >
      {props.children}
    </div>
  );
}

export interface QueuedRowProps extends KitProps {
  /** what it waits for: Next · after Claude Code finishes / Held after Stop / Queued #2 */
  readonly tag: string;
  readonly text: string;
  /** held after a Stop: the tag reads quieter */
  readonly held?: boolean;
  /** two or more: the grip that moves it (a drag of a row's height per step, or ↑/↓) */
  readonly grip?: { readonly part?: PartName; readonly label: string; readonly onMove: (by: number) => void };
  /** QueuedAction's, shown over the end of the text on hover or focus (not on a touch screen: a tap is the menu) */
  readonly actions?: JSX.Element;
  /** ×, always there */
  readonly cancel: { readonly label: string; readonly onClick: () => void };
  /** the row's menu is about to open: a right-click, or a tap on a touch screen (not on its buttons). The caller's
   * ContextMenu around the list opens it there (a tap asks for it with a contextmenu event). */
  readonly onMenu?: () => void;
  /** a double-click on the row (edit it in place) */
  readonly onOpen?: () => void;
  /** being edited: the caller's box in the text's place, and the keys under it (↵ save · esc cancel) */
  readonly edit?: { readonly box: JSX.Element; readonly keys: string } | undefined;
}

const onControl = (e: Event): boolean => e.target instanceof Element && e.target.closest('button, [data-kit="cq-grip"], [data-kit="cq-editbox"]') !== null;
const touch = (): boolean => typeof matchMedia === 'function' && matchMedia('(hover: none)').matches;

/** Drag the grip: the row follows the pointer and the rows it passes make room (v1 dragRow); let go, it moves by the
 * rows it passed. ↑/↓ on the grip moves it one. */
function Grip(props: { readonly part: PartName; readonly label: string; readonly onMove: (by: number) => void }): JSX.Element {
  const d = { y: 0, step: 1, from: 0, to: 0, on: false, row: undefined as HTMLElement | undefined, rows: [] as HTMLElement[] };
  const place = (): void => {
    d.rows.forEach((r, i) => {
      if (r === d.row) return;
      r.style.translate = i > d.from && i <= d.to ? `0 ${-d.step}px` : i < d.from && i >= d.to ? `0 ${d.step}px` : '';
    });
  };
  const end = (): void => {
    if (!d.on) return;
    d.on = false;
    d.row?.closest('[data-kit="cq-list"]')?.removeAttribute('data-sorting');
    d.row?.removeAttribute('data-dragged');
    for (const r of d.rows) r.style.translate = '';
    const by = d.to - d.from;
    d.rows = [];
    if (by !== 0) props.onMove(by);
  };
  return (
    <span
      data-kit="cq-grip"
      data-part={props.part}
      role="button"
      tabindex={0}
      aria-label={props.label}
      title={props.label}
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        const row = (e.currentTarget as HTMLElement).closest<HTMLElement>('[data-kit="cq-row"]');
        const list = row?.closest<HTMLElement>('[data-kit="cq-list"]');
        if (!row || !list) return;
        e.preventDefault();
        d.row = row;
        d.rows = [...list.querySelectorAll<HTMLElement>(':scope > [data-kit="cq-row"]')];
        d.from = d.to = d.rows.indexOf(row);
        d.y = e.clientY;
        // from one row's top to the next (v1 dragRow): the 4px between, and right when one row is taller
        const [r0, r1] = d.rows;
        d.step = (r0 && r1 ? r1.getBoundingClientRect().top - r0.getBoundingClientRect().top : row.getBoundingClientRect().height) || 1;
        d.on = true;
        list.setAttribute('data-sorting', '');
        row.setAttribute('data-dragged', '');
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      }}
      onPointerMove={(e) => {
        if (!d.on || !d.row) return;
        // kept between the first row and the last (v1)
        const dy = Math.max(-d.from * d.step, Math.min((d.rows.length - 1 - d.from) * d.step, e.clientY - d.y));
        d.row.style.translate = `0 ${dy}px`;
        d.to = Math.max(0, Math.min(d.rows.length - 1, d.from + Math.round(dy / d.step)));
        place();
      }}
      onPointerUp={end}
      onPointerCancel={() => {
        d.to = d.from;
        end();
      }}
      onLostPointerCapture={end}
      onKeyDown={(e) => {
        if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
        e.preventDefault();
        props.onMove(e.key === 'ArrowUp' ? -1 : 1);
      }}
    >
      <Icon name="grip" />
    </span>
  );
}

export function QueuedRow(props: QueuedRowProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.queued.row';
  return (
    <div
      data-kit="cq-row"
      data-part={part()}
      data-held={props.held ? '' : undefined}
      data-sortable={props.grip ? '' : undefined}
      data-editing={props.edit ? '' : undefined}
      role="listitem"
      onContextMenu={(e) => {
        // the box being typed in keeps the browser's menu (paste, spelling); anywhere else on the row is its menu (v1)
        if (props.edit) return e.stopPropagation();
        props.onMenu?.();
      }}
      onClick={(e) => {
        if (!props.onMenu || props.edit || onControl(e) || !touch()) return;
        e.currentTarget.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: e.clientX, clientY: e.clientY }));
      }}
      onDblClick={(e) => {
        if (!props.edit && !onControl(e)) props.onOpen?.();
      }}
    >
      <Show when={props.grip && !props.edit ? props.grip : undefined}>{(g) => <Grip part={g().part ?? sub(part(), 'grip')} label={g().label} onMove={(by) => g().onMove(by)} />}</Show>
      <span data-kit="cq-tag" data-part={sub(part(), 'tag')}>
        {props.tag}
      </span>
      <Show
        when={props.edit}
        fallback={
          <span data-kit="cq-text" data-part={sub(part(), 'text')} title={props.text}>
            {props.text}
          </span>
        }
      >
        {(ed) => (
          <span data-kit="cq-editbox" data-part={sub(part(), 'edit')} onPointerDown={(e) => e.stopPropagation()}>
            {ed().box}
            <span data-kit="cq-keys" data-part={sub(part(), 'keys')}>
              {ed().keys}
            </span>
          </span>
        )}
      </Show>
      <Show when={props.actions && !props.edit}>
        <span data-kit="cq-acts" data-part={sub(part(), 'actions')}>
          {props.actions}
        </span>
      </Show>
      <button
        type="button"
        data-kit="cq-x"
        data-part={sub(part(), 'cancel')}
        aria-label={props.cancel.label}
        title={props.cancel.label}
        onClick={(e) => {
          e.stopPropagation();
          props.cancel.onClick();
        }}
      >
        <Icon name="x" />
      </button>
    </div>
  );
}

export interface QueuedActionProps extends KitProps {
  readonly icon: IconName;
  /** its name for a screen reader and its tooltip */
  readonly label: string;
  /** a longer tooltip (Send now: interrupts Claude Code with this follow-up) */
  readonly title?: string;
  /** words beside the icon (v1 `.cq-now`: Send now) */
  readonly text?: string;
  readonly onClick: () => void;
}

/** One of a row's hover actions (v1 `.cq-acts button`): a 20px icon, or an icon and words. */
export function QueuedAction(props: QueuedActionProps): JSX.Element {
  return (
    <button
      type="button"
      data-kit="cq-act"
      data-part={props.part ?? 'ui.queued.action'}
      data-text={props.text ? '' : undefined}
      aria-label={props.label}
      title={props.title ?? props.label}
      onClick={(e) => {
        e.stopPropagation();
        props.onClick();
      }}
    >
      <Icon name={props.icon} />
      <Show when={props.text}>
        <span>{props.text}</span>
      </Show>
    </button>
  );
}
