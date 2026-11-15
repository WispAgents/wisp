// The overlays' phone halves (ADR-0065 build step 5, design/mobile.md §A.4–§A.7), loaded apart (./phone-part.ts): the
// browser never fetches this module. What it holds:
//   - PhoneMenu: a long-press's menu. Android: an M3 modal bottom sheet of the row's actions with *Select* first (the
//     kit's Sheet, so it drags down to close and its × is the twin). iOS: the context menu, the pressed row lifted as
//     its preview over a blurred page and the menu beside it (UIContextMenuInteraction's layout, ./phone-math.ts).
//   - bindSheet: a phone sheet's drag (step 7's bindSheetDrag): down closes (a draft stays); on iOS it steps the
//     detents (large → medium → closed) and a drag up grows it.
//   - bindTip: Android's M3 plain tooltip on a long-press of an icon button (1.5 s); the press then opens nothing.
//   - SelectSheet (step 6): a Select's long list. Android: an M3 modal bottom sheet of radio rows; iOS: a sheet of the
//     choices as a checkmark list (HIG's pushed list for a long set). A choice closes it.
// Words come from props and the kit's strings (t()); every number from the platform tokens, mirrored here only where
// the layout must be decided before the menu draws (the iOS row height, separator and width).
import { createEffect, createMemo, createSignal, For, Match, onCleanup, onMount, Show, Switch, type JSX } from 'solid-js';
import { Portal } from 'solid-js/web';
import { flag, sub, type PartName } from '../core/props.ts';
import { useT } from '../core/i18n.tsx';
import { Icon } from '../content/Icon.tsx';
import { Sheet } from './Dialog.tsx';
import type { MenuEntry, MenuItem, MenuSubmenu } from './menu-entries.ts';
import type { MenuMachineProps } from './menu-machine.tsx';
import type { ContextMenuProps } from './Menu.tsx';
import type { SelectOption } from './Select.tsx';
import { bound } from './lazy-part.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { contextPlace, detentAfter, detentsOf, phoneRows, rowsHeight, SELECT_VALUE, tipPlace, TIP_MS, type Detent, type PhoneRow } from './phone-math.ts';
import { bindSheetDrag } from '../gestures/bind-vertical.ts';
import { all, listen } from '../gestures/bind.ts';
import { GESTURE_SLOP, LONG_PRESS_MS } from '../gestures/gesture-math.ts';
import type { GestureHost } from '../gestures/host.tsx';
import { markGesture } from '../gestures/use.ts';
import { pressTimers, type Schedule } from '../gestures/press-scope.ts';

// the iOS tokens the context menu's place needs before it draws (ios.json: ctx-row-h, ctx-w, ctx-gap)
const IOS_ROW = 44;
const IOS_SEP = 8;
const IOS_W = 250;
const IOS_GAP = 8;

export interface PhoneMenuProps {
  readonly look: 'android' | 'ios';
  readonly items: readonly MenuEntry[];
  readonly onSelect: (value: string) => void;
  readonly onClose: () => void;
  /** the pressed row: the iOS preview, the Android sheet's title */
  readonly target: HTMLElement;
  /** where the finger was across: the iOS menu sits under the near half's edge */
  readonly x: number;
  readonly part: PartName;
  /** Android: *Select* first (the list's selection mode) */
  readonly onSelectMode?: (() => void) | undefined;
}

/** what a long-press lifts as the iOS preview: the pressed row */
const ROW = "[data-kit='swipe'], [data-kit='list-row'], [data-kit='card']";

/** A ContextMenu's machine in the phone apps (Menu.tsx binds it in place of menu-machine's, with the same props): a
 * contextmenu on the area (a long-press dispatches one: Swipe's onLongPress) opens PhoneMenu on the pressed row. */
export function MenuMachine(props: MenuMachineProps): JSX.Element {
  const look = usePlatform();
  const [press, setPress] = createSignal<{ readonly target: HTMLElement; readonly x: number }>();
  const close = (): void => {
    setPress(undefined);
    props.base.onOpenChange?.(false);
  };
  createEffect(() => {
    if (props.base.open === false) setPress(undefined);
  });
  props.bind(() => ({
    onContextMenu: (e: MouseEvent) => {
      e.preventDefault();
      const at = e.target instanceof Element ? e.target : null;
      setPress({ target: (at?.closest<HTMLElement>(ROW) ?? at ?? e.currentTarget) as HTMLElement, x: e.clientX });
      props.base.onOpenChange?.(true);
    },
  }));
  // a long-press before this chunk was there: that press again, now
  onMount(() => bound(undefined, props.pending));
  return (
    <Show when={press()}>
      {(p) => (
        <PhoneMenu
          look={look() === 'ios' ? 'ios' : 'android'}
          items={props.base.items}
          onSelect={props.base.onSelect}
          onClose={close}
          target={p().target}
          x={p().x}
          part={props.part}
          onSelectMode={(props.base as ContextMenuProps).onSelectMode}
        />
      )}
    </Show>
  );
}

export function PhoneMenu(props: PhoneMenuProps): JSX.Element {
  return (
    <Show when={props.look === 'ios'} fallback={<ActionSheet {...props} />}>
      <ContextPreview {...props} />
    </Show>
  );
}

/** The pressed row's name: its title's text, else its own. */
const titleOf = (el: HTMLElement): string => (el.querySelector("[data-kit='list-row-title']")?.textContent ?? el.textContent ?? '').trim();

/** Rows with submenus drilled into one level (v1 #786): a submenu's rows under a Back row. */
function useRows(props: PhoneMenuProps, select: () => string | undefined) {
  const [pane, setPane] = createSignal<MenuSubmenu>();
  const rows = createMemo<PhoneRow[]>(() => {
    const p = pane();
    return p ? phoneRows(p.items) : phoneRows(props.items, select());
  });
  const choose = (item: MenuItem): void => {
    if (item.disabled) return;
    props.onClose();
    if (item.value === SELECT_VALUE) props.onSelectMode?.();
    else props.onSelect(item.value);
  };
  return { pane, setPane, rows, choose };
}

function Rows(props: { readonly rows: readonly PhoneRow[]; readonly part: PartName; readonly kit: 'sheet' | 'context'; readonly choose: (i: MenuItem) => void; readonly drill: (s: MenuSubmenu) => void }): JSX.Element {
  return (
    <For each={props.rows}>
      {(row) => (
        <Switch>
          <Match when={row.kind === 'separator'}>
            <div role="separator" data-kit={`${props.kit}-separator`} />
          </Match>
          <Match when={row.kind === 'label' && row}>{(r) => <div role="presentation" data-kit={`${props.kit}-label`}>{r().label}</div>}</Match>
          <Match when={row.kind === 'sub' && row}>
            {(r) => (
              <button type="button" role="menuitem" aria-haspopup="menu" data-kit={`${props.kit}-item`} data-part={r().entry.part ?? sub(props.part, 'sub')} data-value={r().entry.value} onClick={() => props.drill(r().entry)}>
                <Show when={r().entry.icon}>{(name) => <Icon name={name()} size="md" />}</Show>
                <span data-kit={`${props.kit}-item-label`} data-text="control">
                  {r().entry.label}
                </span>
                <Icon name="chevron" size="sm" tone="faint" />
              </button>
            )}
          </Match>
          <Match when={row.kind === 'item' && row}>
            {(r) => (
              <button
                type="button"
                role={r().item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'}
                aria-checked={r().item.checked === undefined ? undefined : r().item.checked}
                disabled={r().item.disabled}
                data-kit={`${props.kit}-item`}
                data-part={r().item.part ?? sub(props.part, 'item')}
                data-value={r().item.value}
                data-danger={flag(r().item.danger)}
                onClick={() => props.choose(r().item)}
              >
                <Show when={r().item.icon}>{(name) => <Icon name={name()} size="md" />}</Show>
                <span data-kit={`${props.kit}-item-text`}>
                  <span data-kit={`${props.kit}-item-label`} data-text="control">
                    {r().item.label}
                  </span>
                  <Show when={r().item.hint}>{(hint) => <span data-kit={`${props.kit}-item-hint`}>{hint()}</span>}</Show>
                </span>
                <Show when={r().item.checked}>
                  <Icon name="check" size="sm" tone="accent-ink" />
                </Show>
              </button>
            )}
          </Match>
        </Switch>
      )}
    </For>
  );
}

/** Android: the row's actions in an M3 modal bottom sheet, *Select* first, the row's title as its headline. */
function ActionSheet(props: PhoneMenuProps): JSX.Element {
  const t = useT();
  const title = titleOf(props.target) || t('gesture.actions');
  const m = useRows(props, () => (props.onSelectMode ? t('select') : undefined));
  return (
    <Sheet open title={title} onOpenChange={(open) => !open && props.onClose()} part={sub(props.part, 'sheet')}>
      <div role="menu" aria-label={title} data-kit="sheet-menu" data-part={sub(props.part, 'list')}>
        <Show when={m.pane()}>
          {(p) => (
            <button type="button" role="menuitem" data-kit="sheet-item" data-part={sub(props.part, 'back')} data-back="" onClick={() => m.setPane(undefined)}>
              <Icon name="back" size="md" />
              <span data-kit="sheet-item-label" data-text="control">
                {p().label}
              </span>
            </button>
          )}
        </Show>
        <Rows rows={m.rows()} part={props.part} kit="sheet" choose={m.choose} drill={m.setPane} />
      </div>
    </Sheet>
  );
}

/** iOS: the context menu. The row lifts in place over a blurred page; the menu opens under it (or over it, or the
 * preview slides up to make room); a tap outside, Escape or a choice closes it. */
function ContextPreview(props: PhoneMenuProps): JSX.Element {
  const t = useT();
  const m = useRows(props, () => undefined);
  // one layout read as it opens: where the row is
  const r = props.target.getBoundingClientRect();
  const row = { top: r.top, left: r.left, width: r.width, height: r.height };
  const vp = { width: window.innerWidth, height: window.innerHeight, safeTop: 0, safeBottom: 0 };
  const place = createMemo(() => contextPlace(row, vp, { width: Math.min(IOS_W, vp.width - 16), height: rowsHeight(m.rows(), IOS_ROW, IOS_SEP) + (m.pane() ? IOS_ROW : 0) }, IOS_GAP, props.x));
  // the preview is a picture of the row: nothing in it takes focus or counts as the row's controls and marks
  const clone = props.target.cloneNode(true) as HTMLElement;
  clone.inert = true;
  for (const el of [clone, ...clone.querySelectorAll('[id], [data-gesture], [data-part]')]) for (const a of ['id', 'data-gesture', 'data-twin', 'data-part']) el.removeAttribute(a);
  let menu: HTMLDivElement | undefined;
  // focus goes into the menu, and back to where it was (or the row) when it closes
  const before = document.activeElement instanceof HTMLElement ? document.activeElement : undefined;
  const pressed = props.target;
  const escape = (e: KeyboardEvent): void => {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    props.onClose();
  };
  onMount(() => {
    menu?.querySelector<HTMLElement>("[data-kit='context-item']:not([disabled])")?.focus();
    document.addEventListener('keydown', escape, true);
  });
  onCleanup(() => {
    document.removeEventListener('keydown', escape, true);
    const back = before?.isConnected && before !== document.body ? before : pressed;
    if (back.isConnected) back.focus({ preventScroll: true });
  });
  const keys = (e: KeyboardEvent): void => {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const items = [...(menu?.querySelectorAll<HTMLElement>("[data-kit='context-item']:not([disabled])") ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    items[(at + (e.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length]?.focus();
  };
  return (
    <Portal>
      <div data-kit="context-backdrop" data-part={sub(props.part, 'backdrop')} onClick={() => props.onClose()} />
      <div
        aria-hidden="true"
        data-kit="context-preview"
        data-part={sub(props.part, 'preview')}
        style={{ '--k-cx-top': `${place().previewTop}px`, '--k-cx-left': `${row.left}px`, '--k-cx-w': `${row.width}px`, '--k-cx-h': `${row.height}px` }}
        ref={(el) => el.append(clone)}
        onClick={() => props.onClose()}
      />
      <div
        ref={menu}
        role="menu"
        aria-modal="true"
        aria-label={titleOf(props.target) || t('gesture.actions')}
        data-kit="context-menu"
        data-part={props.part}
        data-below={flag(place().below)}
        style={{ '--k-cx-mtop': `${place().menuTop}px`, '--k-cx-mleft': `${place().menuLeft}px` }}
        onKeyDown={keys}
      >
        <Show when={m.pane()}>
          {(p) => (
            <button type="button" role="menuitem" data-kit="context-item" data-part={sub(props.part, 'back')} data-back="" onClick={() => m.setPane(undefined)}>
              <span data-kit="context-item-label" data-text="control">
                {p().label}
              </span>
              <Icon name="back" size="sm" />
            </button>
          )}
        </Show>
        <Rows rows={m.rows()} part={props.part} kit="context" choose={m.choose} drill={m.setPane} />
      </div>
    </Portal>
  );
}

// -------------------------------------------------------------------------------------------------------------------

export interface SheetBind {
  readonly host: GestureHost;
  readonly look: 'android' | 'ios';
  /** the iOS sheet's detents and where it is now (Android: one, closing only) */
  readonly detents: readonly Detent[];
  readonly detent: () => Detent;
  readonly setDetent: (d: Detent) => void;
  readonly onClose: () => void;
  /** the close button's words, the screen reader's action */
  readonly closeLabel: string;
}

/** A phone sheet's drag: it follows the finger (`--k-sheet-dy`), closes past the line or on a flick (the caller keeps
 * the draft); on iOS a drag down from large settles on medium and a drag up grows it. Returns the stop. */
export function bindSheet(el: HTMLElement, o: SheetBind): () => void {
  markGesture(el, 'sheet.drag-down');
  const body = (): HTMLElement | undefined => el.querySelector<HTMLElement>("[data-kit='dialog-body']") ?? undefined;
  const step = (dir: 'down' | 'up'): void => {
    const next = o.look === 'ios' ? detentAfter(o.detents, o.detent(), dir) : dir === 'down' ? 'close' : o.detent();
    if (next === 'close') o.onClose();
    else if (next !== o.detent()) {
      o.setDetent(next);
      o.host.haptic('select');
    }
  };
  const canLift = (): boolean => o.look === 'ios' && o.detent() !== 'large' && o.detents.includes('large');
  const stop = bindSheetDrag(el, {
    host: o.host,
    content: body,
    onClose: () => step('down'),
    onDrag: (dy) => {
      el.toggleAttribute('data-dragging', dy !== 0);
      el.style.setProperty('--k-sheet-dy', `${dy}px`);
    },
    ...(o.look === 'ios' ? { onLift: () => step('up'), canLift } : {}),
  });
  // iOS: scrolling the content at medium grows the sheet first (UISheetPresentationController)
  const grow = (): void => {
    if (canLift() && (body()?.scrollTop ?? 0) > 0) step('up');
  };
  const content = body();
  content?.addEventListener('scroll', grow, { passive: true });
  const off = o.host.actions?.(el, [{ label: o.closeLabel, run: o.onClose }]);
  return () => {
    stop();
    content?.removeEventListener('scroll', grow);
    off?.();
  };
}

/** What Dialog.tsx hands a phone surface as it draws. */
export interface PhoneSurface {
  readonly host: GestureHost;
  readonly look: 'android' | 'ios';
  readonly kind: 'dialog' | 'sheet';
  readonly role?: 'dialog' | 'alertdialog' | undefined;
  readonly size?: string | undefined;
  readonly bare?: boolean | undefined;
  readonly place?: 'top' | undefined;
  readonly detents?: readonly Detent[] | undefined;
  readonly part: PartName;
  readonly onClose: () => void;
  readonly closeLabel: string;
}

/** A confirm on a phone: `alertdialog`, or a small dialog (M3 basic dialog without ×, the iOS alert). */
export const isAlert = (o: Pick<PhoneSurface, 'kind' | 'role' | 'size'>): boolean => o.kind === 'dialog' && (o.role === 'alertdialog' || o.size === 'sm');

/** A phone dialog's or sheet's surface (Dialog.tsx draws the browser's; this gives it the platform's shape): a confirm
 * is marked `data-alert`; on iOS any other dialog becomes a sheet (HIG: a modal task is a sheet; its part stays the
 * dialog's); a sheet gets the twin's name the gesture registry reads (`ui.sheet.close`) on its × whatever its part,
 * the iOS detents (`data-detent`, opening at the first; a tap on the grabber steps them) and the drag. Returns the stop. */
export function phoneSurface(el: HTMLElement, o: PhoneSurface): () => void {
  if (isAlert(o)) {
    el.setAttribute('data-alert', '');
    return () => undefined;
  }
  if (o.kind === 'dialog') {
    if (o.look === 'android' || o.bare || o.place === 'top') return () => undefined;
    el.setAttribute('data-kit', 'sheet');
    el.parentElement?.setAttribute('data-sheet', '');
    el.parentElement?.previousElementSibling?.setAttribute('data-sheet', '');
    const grabber = document.createElement('span');
    grabber.setAttribute('data-kit', 'sheet-handle');
    grabber.setAttribute('aria-hidden', 'true');
    el.prepend(grabber);
  }
  const close = el.querySelector("[data-kit='dialog-head'] > [data-kit='icon-button']");
  if (close && o.part !== 'ui.sheet') close.setAttribute('data-part', `${sub(o.part, 'close')} ui.sheet.close`);
  // a dialog drawn as a sheet holds a form: it opens large unless it names its detents
  const detents = detentsOf(o.detents ?? (o.kind === 'dialog' ? ['large'] : undefined));
  let at: Detent = detents[0] ?? 'large';
  const setDetent = (d: Detent): void => {
    at = d;
    if (o.look === 'ios') el.setAttribute('data-detent', d);
  };
  setDetent(at);
  const grabber = el.querySelector<HTMLElement>("[data-kit='sheet-handle']");
  const tap = (): void => setDetent(at === 'large' ? 'medium' : 'large');
  const steps = o.look === 'ios' && detents.length > 1;
  if (steps) grabber?.addEventListener('click', tap);
  const stop = bindSheet(el, { host: o.host, look: o.look, detents, detent: () => at, setDetent, onClose: o.onClose, closeLabel: o.closeLabel });
  return () => {
    stop();
    if (steps) grabber?.removeEventListener('click', tap);
  };
}

// -------------------------------------------------------------------------------------------------------------------

export interface TipBind {
  readonly host: GestureHost;
  readonly label: () => string;
  readonly hint?: () => string | undefined;
  readonly schedule?: Schedule;
}

/** Android: a long-press on the trigger shows its M3 plain tooltip for TIP_MS after the finger lifts; that press opens
 * nothing (its click is swallowed). One at a time; it never takes focus. Returns the stop. */
export function bindTip(el: HTMLElement, o: TipBind): () => void {
  const timers = pressTimers();
  const schedule = o.schedule ?? timers.schedule;
  let press: { x: number; y: number; cancel: () => void } | undefined;
  let tip: HTMLElement | undefined;
  let hideAfter: (() => void) | undefined;
  let fired = false;
  const hide = (): void => {
    hideAfter?.();
    tip?.remove();
    tip = undefined;
  };
  const show = (): void => {
    press = undefined;
    fired = true;
    hide();
    o.host.haptic('tap');
    const box = document.createElement('div');
    box.setAttribute('role', 'tooltip');
    box.setAttribute('data-kit', 'plain-tip');
    box.textContent = o.label();
    const hint = o.hint?.();
    if (hint) {
      const h = document.createElement('span');
      h.setAttribute('data-kit', 'plain-tip-hint');
      h.textContent = hint;
      box.append(' ', h);
    }
    document.body.append(box);
    // two layout reads as it shows: the anchor and the tip, then one write
    const a = el.getBoundingClientRect();
    const b = box.getBoundingClientRect();
    const at = tipPlace({ top: a.top, left: a.left, width: a.width, height: a.height }, { width: b.width, height: b.height }, { width: window.innerWidth, height: window.innerHeight, safeTop: 0, safeBottom: 0 }, 4);
    box.style.setProperty('--k-tip-top', `${at.top}px`);
    box.style.setProperty('--k-tip-left', `${at.left}px`);
    tip = box;
  };
  const up = (): void => {
    press?.cancel();
    press = undefined;
    if (tip) hideAfter = schedule(hide, TIP_MS);
  };
  return all([
    timers.dispose,
    hide,
    listen(el, 'pointerdown', (e) => {
      press?.cancel();
      fired = false;
      if (e.pointerType !== 'touch' || !e.isPrimary) return;
      press = { x: e.clientX, y: e.clientY, cancel: schedule(show, LONG_PRESS_MS) };
    }),
    listen(el, 'pointermove', (e) => {
      if (press && Math.hypot(e.clientX - press.x, e.clientY - press.y) > GESTURE_SLOP) up();
    }),
    listen(el, 'pointerup', up),
    listen(el, 'pointercancel', up),
    listen(el, 'contextmenu', (e) => {
      if (press || fired) e.preventDefault();
    }),
    listen(el, 'keydown', () => void (fired = false), { capture: true }),
    listen(
      el,
      'click',
      (e) => {
        if (!fired) return;
        fired = false;
        e.preventDefault();
        e.stopPropagation();
      },
      { capture: true },
    ),
  ]);
}

export interface SelectSheetProps {
  readonly look: 'android' | 'ios';
  readonly items: readonly SelectOption[];
  readonly value: string | undefined;
  /** the field's name (its aria-label, its Field's label, its placeholder) */
  readonly title: string | undefined;
  readonly part: PartName;
  readonly onChoose: (value: string) => void;
  readonly onClose: () => void;
}

/** A Select's long list in the phone apps: Android's bottom sheet of radio rows, iOS's checkmark list in a sheet. The
 * picked row takes focus first; a choice (or a drag down, Escape, the scrim) closes it, focus back on the button. */
export function SelectSheet(props: SelectSheetProps): JSX.Element {
  const t = useT();
  const title = (): string => props.title?.trim() || t('select');
  let list: HTMLDivElement | undefined;
  const pick = (item: SelectOption): void => {
    props.onChoose(item.value);
    props.onClose();
  };
  const keys = (e: KeyboardEvent): void => {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const rows = [...(list?.querySelectorAll<HTMLElement>("[data-kit='select-sheet-item']:not([disabled])") ?? [])];
    const at = rows.indexOf(document.activeElement as HTMLElement);
    const next = at < 0 ? (e.key === 'ArrowDown' ? 0 : rows.length - 1) : (at + (e.key === 'ArrowDown' ? 1 : rows.length - 1)) % rows.length;
    rows[next]?.focus();
  };
  return (
    <Sheet
      open
      title={title()}
      onOpenChange={(open) => !open && props.onClose()}
      part={sub(props.part, 'sheet')}
      initialFocus={() => list?.querySelector<HTMLElement>("[data-kit='select-sheet-item'][aria-checked='true']:not([disabled])") ?? list?.querySelector<HTMLElement>("[data-kit='select-sheet-item']:not([disabled])") ?? null}
    >
      <div ref={list} role="radiogroup" aria-label={title()} data-kit="select-sheet" data-look={props.look} data-part={sub(props.part, 'list')} onKeyDown={keys}>
        <For each={props.items}>
          {(item, i) => (
            <>
              <Show when={item.group && props.items[i() - 1]?.group !== item.group}>
                <div role="presentation" data-kit="select-sheet-group">
                  {item.group}
                </div>
              </Show>
              <button
                type="button"
                role="radio"
                aria-checked={item.value === props.value}
                disabled={item.disabled}
                data-kit="select-sheet-item"
                data-part={sub(props.part, 'option')}
                data-value={item.value}
                onClick={() => pick(item)}
              >
                <Show when={props.look === 'android'}>
                  <span data-kit="select-sheet-radio" aria-hidden="true" data-checked={flag(item.value === props.value)} />
                </Show>
                <span data-kit="select-sheet-label" data-text="control">
                  {item.label}
                </span>
                <Show when={props.look === 'ios' && item.value === props.value}>
                  <Icon name="check" size="md" tone="accent-ink" />
                </Show>
              </button>
            </>
          )}
        </For>
      </div>
    </Sheet>
  );
}
