// What is scheduled on a task, over its reply box (v1 #796 later.js followUpItems, later.css `.cq.later`): one compact
// list, a joined box with a short row each: its icon and time in a fixed column so the texts line up, what it says
// (one line), then the pen (shown on hover) and ×. Who set it and "in 18 min" are the row's tooltip. Past a few rows
// the caller shows the first ones and a ScheduledMore row ("3 more scheduled · next 14:52", Show less). Its CSS
// loads with it (scheduled.css), not on the first page.
import { Show, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName, type WithChildren } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import '../css/scheduled.css';

export interface ScheduledListProps extends KitProps, WithChildren {
  readonly label: string;
}

/** The joined box. Its children are ScheduledRow, ScheduledEdit and ScheduledMore. */
export function ScheduledList(props: ScheduledListProps): JSX.Element {
  return (
    <div data-kit="sched-list" data-part={props.part ?? 'ui.scheduled'} role="list" aria-label={props.label}>
      {props.children}
    </div>
  );
}

export interface ScheduledRowProps extends KitProps {
  readonly icon: IconName;
  /** 14:45, Tue 9:00: the fixed column */
  readonly time: string;
  readonly text: string;
  /** Follow-up · In 18 min · Tue 4 Nov, 14:32 / Set by you */
  readonly tip: string;
  /** held for the plan or the computer: the tag reads quieter */
  readonly held?: boolean;
  /** files that go with it, and the tooltip that says so */
  readonly files?: { readonly count: number; readonly tip: string };
  /** the row's menu is about to open: a right-click, a click, Enter or Space on the row (not on its buttons). The
   * caller's ContextMenu around the list opens it there (a click or a key asks for it with a contextmenu event). */
  readonly onMenu?: () => void;
  /** ScheduledButton's, last on the row */
  readonly actions?: JSX.Element;
}

export function ScheduledRow(props: ScheduledRowProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.scheduled.row';
  const onButton = (e: Event): boolean => e.target instanceof Element && e.target.closest('button') !== null;
  const ask = (el: HTMLElement, x: number, y: number): void => {
    el.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: x, clientY: y }));
  };
  return (
    <div
      data-kit="sched-row"
      data-part={part()}
      data-held={props.held ? '' : undefined}
      role="listitem"
      tabindex={props.onMenu ? 0 : undefined}
      onContextMenu={(e) => {
        if (!onButton(e)) props.onMenu?.();
      }}
      onClick={(e) => {
        if (props.onMenu && !onButton(e)) ask(e.currentTarget, e.clientX, e.clientY);
      }}
      onKeyDown={(e) => {
        if (!props.onMenu || onButton(e) || (e.key !== 'Enter' && e.key !== ' ')) return;
        e.preventDefault();
        const r = e.currentTarget.getBoundingClientRect();
        ask(e.currentTarget, r.left + r.width / 2, r.bottom);
      }}
    >
      <span data-kit="sched-tag" data-part={sub(part(), 'tag')} title={props.tip}>
        <Icon name={props.icon} />
        <span data-kit="sched-at">{props.time}</span>
        <Show when={props.files}>
          {(f) => (
            <span data-kit="sched-files" data-part={sub(part(), 'files')} title={f().tip}>
              <Icon name="clip" />
              {f().count}
            </span>
          )}
        </Show>
      </span>
      <span data-kit="sched-text" data-part={sub(part(), 'text')} title={props.tip}>
        {props.text}
      </span>
      <Show when={props.actions}>
        <span data-kit="sched-acts" data-part={sub(part(), 'actions')}>
          {props.actions}
        </span>
      </Show>
    </div>
  );
}

export interface ScheduledButtonProps extends KitProps {
  readonly icon: IconName;
  readonly label: string;
  readonly onClick: () => void;
  /** shown only while the row is hovered or focused (always on touch): the pen */
  readonly reveal?: boolean;
  /** red on hover: × */
  readonly danger?: boolean;
}

/** A row's 18px icon button (v1 `.cq.later .cq-pen`, `.att-x`). */
export function ScheduledButton(props: ScheduledButtonProps): JSX.Element {
  return (
    <button
      type="button"
      data-kit="sched-btn"
      data-part={props.part ?? 'ui.scheduled.button'}
      data-reveal={props.reveal ? '' : undefined}
      data-danger={props.danger ? '' : undefined}
      aria-label={props.label}
      title={props.label}
      onClick={(e) => {
        e.stopPropagation();
        props.onClick();
      }}
    >
      <Icon name={props.icon} />
    </button>
  );
}

/** A row being edited in place: the caller's text box, in the row's place. */
export function ScheduledEdit(props: KitProps & WithChildren): JSX.Element {
  return (
    <div data-kit="sched-row" data-editing="" data-part={props.part ?? 'ui.scheduled.edit'} role="listitem">
      {props.children}
    </div>
  );
}

export interface ScheduledMoreProps extends KitProps {
  readonly open: boolean;
  /** 3 more scheduled · next 14:52, or Show less */
  readonly label: string;
  readonly onToggle: () => void;
}

/** The last row past a few: opens the rest, or closes them again. */
export function ScheduledMore(props: ScheduledMoreProps): JSX.Element {
  return (
    <div data-kit="sched-row" data-more="" data-part={sub(props.part ?? 'ui.scheduled.more', 'row')} role="listitem">
      <button type="button" data-kit="sched-more" data-part={props.part ?? 'ui.scheduled.more'} aria-expanded={props.open} onClick={() => props.onToggle()}>
        <Icon name="chevron" />
        {props.label}
      </button>
    </div>
  );
}
