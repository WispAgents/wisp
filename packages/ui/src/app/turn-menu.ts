// An answer's menu on a phone (UX 2026-11-18 phone #8; QA r6 #15, r8 #2, r9 #2): its ⋯ and a long-press on the answer
// open one menu of what the answer offers. Copy (its words), Comment… (on the block held: that block's own Comment),
// the trail's tools (Agent notes, Restore, Fork) and what has its own control on the trail (Good / Bad result). The
// hold's menu opens where the finger was, kept on screen; it opens once the finger is up (opened under it, the lift was
// a press outside it). The browser draws none of it: its trail shows every tool.
import { createSignal, type Accessor } from 'solid-js';
import type { MenuPoint } from '../controls/Menu.tsx';
import type { MenuEntry } from '../controls/menu-entries.ts';
import type { IconName } from '../content/icons.ts';
import { useT } from '../core/i18n.tsx';
import { bindLongPress } from '../gestures/bind.ts';
import type { GestureHost } from '../gestures/host.tsx';
import { useGesture } from '../gestures/use.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';
import type { Act } from './act.tsx';

export interface TurnTool {
  readonly id: string;
  readonly icon: IconName;
  readonly label: string;
  readonly part?: string;
  readonly act?: Act;
  readonly on?: boolean;
}

/** What a phone's answer menu holds beyond the trail's tools, in the shell's words. Additive (QA r9 #2). */
export interface TurnPhoneMenu {
  /** Copy: the answer's words, handed to `onCopy` (the platform's clipboard) */
  readonly copy?: { readonly label: string; readonly onCopy: (text: string) => void };
  /** Comment…'s label: offered by a hold on a block that takes comments, it opens that block's Comment */
  readonly comment?: string;
  /** rows whose control is on the trail already (Good / Bad result), after the tools */
  readonly more?: readonly TurnTool[];
}

const COPY = 'turn:copy';
const COMMENT = 'turn:comment';

export function useTurnMenu(props: { readonly tools: readonly TurnTool[]; readonly phoneMenu?: TurnPhoneMenu | undefined }) {
  const look = usePlatform();
  const kitT = useT();
  const [open, setOpen] = createSignal(false);
  // where the hold's menu opens (the ⋯ opens it at the ⋯) and the block it held: both forgotten as it closes
  const [from, setFrom] = createSignal<MenuPoint | undefined>();
  const [block, setBlock] = createSignal<HTMLElement | undefined>();
  let turnEl: HTMLElement | undefined;
  const folds = (): boolean => look() !== 'web' && props.tools.length > 0;
  const items = (): MenuEntry[] => {
    const m = props.phoneMenu;
    const row = (x: TurnTool): MenuEntry => ({ value: x.id, label: x.label, icon: x.icon, ...(x.on !== undefined && { checked: x.on }), ...(x.act?.pending && { disabled: true }) });
    return [
      ...(m?.copy ? [{ value: COPY, label: m.copy.label, icon: 'copy' as const }] : []),
      ...(m?.comment && block() ? [{ value: COMMENT, label: m.comment, icon: 'comment' as const }] : []),
      ...props.tools.map(row),
      ...(m?.more ?? []).map(row),
    ];
  };
  const pick = (id: string): void => {
    if (id === COPY) return props.phoneMenu?.copy?.onCopy(answerText(turnEl));
    // the block held as it opened (the menu has closed, and forgotten it, by the time it says what was picked); its
    // Comment is pressed once the menu is gone (its card takes the focus the menu gives back)
    if (id === COMMENT) {
      const add = held?.querySelector<HTMLElement>(":scope > [data-kit='cmt-add']");
      return void requestAnimationFrame(() => add?.click());
    }
    [...props.tools, ...(props.phoneMenu?.more ?? [])].find((x) => x.id === id)?.act?.onClick?.();
  };
  let held: HTMLElement | undefined;
  const openAt = (at: MenuPoint, on?: HTMLElement): void => {
    held = on;
    setFrom(at);
    setBlock(on);
    setOpen(true);
  };
  const onOpenChange = (o: boolean): void => {
    setOpen(o);
    if (o) return;
    setFrom(undefined);
    setBlock(undefined);
  };
  const bind = useGesture(
    'turn.long-press',
    (el, host) => {
      turnEl = el;
      return bindHold(el, host, (at, on) => {
        if (!folds()) return false;
        openAt(at, on);
        return true;
      });
    },
    // a screen reader's action opens it over the answer's visible top
    () => (folds() ? [{ label: kitT('gesture.actions'), run: () => (turnEl ? openAt(visibleTop(turnEl)) : onOpenChange(true)) }] : []),
  );
  return { bind, folds, items, pick, open: open as Accessor<boolean>, from, onOpenChange };
}

/** The hold on an answer: the menu opens once the finger is up, where it was, on the block it held (when that block
 * takes comments); the text the browser selected under the hold is let go, its bubble giving way to the menu. `open`
 * says whether there was a menu to open. */
function bindHold(el: HTMLElement, host: GestureHost, open: (at: MenuPoint, on?: HTMLElement) => boolean): () => void {
  let pressed: EventTarget | null = null;
  const down = (e: PointerEvent): void => void (pressed = e.target);
  el.addEventListener('pointerdown', down, true);
  const stop = bindLongPress(el, {
    host,
    onLift: true,
    // only a hold that opened the menu lets the selection go (no menu: the hold is the browser's, selecting words)
    onLongPress: (at) => void (open(at, heldBlock(pressed)) && getSelection()?.removeAllRanges()),
  });
  return () => {
    el.removeEventListener('pointerdown', down, true);
    stop();
  };
}

/** the block a hold was on, when it takes comments (it has its own Comment) */
function heldBlock(target: EventTarget | null): HTMLElement | undefined {
  const b = target instanceof Element ? target.closest<HTMLElement>("[data-kit='answer-block']") : null;
  return b?.querySelector(":scope > [data-kit='cmt-add']") ? b : undefined;
}

/** the answer's words, without its controls' (a block's Comment, a code block's Copy) */
function answerText(turn: HTMLElement | undefined): string {
  const answer = turn?.querySelector<HTMLElement>("[data-kit='presentation']");
  if (!answer) return '';
  const copy = answer.cloneNode(true) as HTMLElement;
  for (const b of copy.querySelectorAll('button')) b.remove();
  return (copy.innerText || copy.textContent || '').trim();
}

/** the top of `el` that is on screen, a little in: where a menu opened for it by a screen reader goes */
function visibleTop(el: HTMLElement): MenuPoint {
  const r = el.getBoundingClientRect();
  return { x: r.left + 16, y: Math.min(Math.max(r.top, 0), innerHeight) + 16 };
}
