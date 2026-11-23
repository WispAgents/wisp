// Where the task's page holds the reader while it changes under them (v1 tail.js, turns.js renderBody): pictures growing,
// the reply box growing, a turn landing, streaming output. Every read comes before the one write in a frame.
import { atTail, dockShift, mediaShift, turnsShift, type Grew } from './media-shift.ts';
import { awayFromEnd, END_PX, follow, FOLLOWING, PAUSED, type FollowEvent, type FollowState } from './follow.ts';

const PENDING = Symbol('pending');
const UP_KEYS = new Set(['PageUp', 'Home', 'ArrowUp']);
const DOWN_KEYS = new Set(['PageDown', 'ArrowDown']);

const editable = (t: EventTarget | null): boolean => t instanceof HTMLElement && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName));
const maxOf = (el: HTMLElement): number => Math.max(0, el.scrollHeight - el.clientHeight);
const distOf = (el: HTMLElement): number => Math.max(0, maxOf(el) - el.scrollTop);
const barOf = (body: HTMLElement): number => body.querySelector<HTMLElement>('[data-kit="it-bar"]')?.offsetHeight ?? 0;

/** A wheel or touch inside a nested scroller (a code block, a step's output, the reply textarea) moves that box when it
 *  still has travel in `dir` or isolates overscroll — it never chains to the task page. */
function nestedCanScroll(target: EventTarget | null, body: HTMLElement, dir: 'up' | 'down'): boolean {
  for (let el = target instanceof Element ? target : null; el && el !== body; el = el.parentElement) {
    if (el.scrollHeight <= el.clientHeight + 1) continue;
    const cs = getComputedStyle(el);
    if (!/(auto|scroll)/.test(cs.overflowY)) continue;
    if (cs.overscrollBehaviorY === 'contain' || cs.overscrollBehaviorY === 'none') return true;
    if (dir === 'up' && el.scrollTop > 0) return true;
    if (dir === 'down' && el.scrollTop + el.clientHeight < el.scrollHeight - 1) return true;
  }
  return false;
}

/** Only clicks/keys on interactive disclosure controls inside the turns can open a fold by hand; clicking plain text
 *  to focus the window or clear a selection never suspends streaming follow. */
function isInteractiveIn(target: EventTarget | null, root: HTMLElement): boolean {
  if (!(target instanceof Element) || !root.contains(target)) return false;
  const hit = target.closest('button, summary, [role="button"], [aria-expanded], details');
  return hit !== null && root.contains(hit);
}

export interface Held {
  readonly stop: () => void;
  /** Pin to the bottom at once and follow from here (Jump to latest, follow-up, initial Chat landing). */
  readonly toEnd: () => void;
  /** Place the view at `top` without fighting follow state (kept place restore, Jump to top). */
  readonly place: (top: number, shouldFollow?: boolean) => void;
  /** Whether the view is currently following the end. */
  readonly following: () => boolean;
}

interface InputBindings {
  readonly isFollowing: () => boolean;
  readonly onUp: () => void;
  readonly onEnd: () => void;
  readonly onScroll: () => void;
  readonly onScrollbar: (dragging: boolean) => void;
}

function bindScrollInputs(body: HTMLElement, turnsEl: HTMLElement | null, b: InputBindings): () => void {
  let touchY: number | undefined;
  let draggingBar = false;
  const onWheel = (e: WheelEvent): void => {
    if (e.ctrlKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
    if (e.deltaY < 0) {
      if (body.scrollTop > 0 && !nestedCanScroll(e.target, body, 'up')) b.onUp();
    } else if (e.deltaY > 0 && !b.isFollowing() && distOf(body) <= END_PX && !nestedCanScroll(e.target, body, 'down')) {
      b.onEnd();
    }
  };
  const onTouchStart = (e: TouchEvent): void => {
    touchY = e.touches.length === 1 ? e.touches[0]?.clientY : undefined;
  };
  const onTouchMove = (e: TouchEvent): void => {
    const t = e.touches[0];
    if (touchY === undefined || e.touches.length !== 1 || !t) return void (touchY = undefined);
    const dy = t.clientY - touchY;
    touchY = t.clientY;
    // dragging finger down scrolls content up toward older output; dragging finger up scrolls down toward latest
    if (dy > 2 && body.scrollTop > 0 && !nestedCanScroll(e.target, body, 'up')) b.onUp();
    else if (dy < -2 && !b.isFollowing() && distOf(body) <= END_PX && !nestedCanScroll(e.target, body, 'down')) b.onEnd();
  };
  const onTouchEnd = (): void => {
    touchY = undefined;
  };
  const onPointerDown = (e: PointerEvent): void => {
    if (e.target !== body || e.pointerType === 'touch') return;
    const r = body.getBoundingClientRect();
    if (e.clientX >= r.left + body.clientWidth || e.clientX <= r.left + body.clientLeft) {
      draggingBar = true;
      b.onScrollbar(true);
    }
  };
  const onPointerUp = (): void => {
    if (!draggingBar) return;
    draggingBar = false;
    b.onScrollbar(false);
  };
  const onKey = (e: KeyboardEvent): void => {
    if (e.altKey || e.metaKey || e.ctrlKey || e.isComposing || editable(e.target) || !body.isConnected) return;
    const t = e.target;
    if (t !== document.body && t !== document.documentElement && !(t instanceof Node && body.contains(t))) return;
    if (UP_KEYS.has(e.key) || (e.key === ' ' && e.shiftKey)) {
      if (body.scrollTop > 0) b.onUp();
    } else if (e.key === 'End') {
      b.onEnd();
    } else if ((DOWN_KEYS.has(e.key) || (e.key === ' ' && !e.shiftKey)) && !b.isFollowing() && distOf(body) <= END_PX) {
      b.onEnd();
    }
  };
  // selecting words while output streams under them: the page stops moving them away (opencode)
  const onSelect = (): void => {
    if (!b.isFollowing() || !turnsEl?.querySelector('[data-live]')) return;
    const sel = document.getSelection();
    if (sel && !sel.isCollapsed && sel.anchorNode && turnsEl.contains(sel.anchorNode)) b.onUp();
  };
  body.addEventListener('scroll', b.onScroll, { passive: true });
  body.addEventListener('wheel', onWheel, { passive: true });
  body.addEventListener('touchstart', onTouchStart, { passive: true });
  body.addEventListener('touchmove', onTouchMove, { passive: true });
  body.addEventListener('touchend', onTouchEnd, { passive: true });
  body.addEventListener('touchcancel', onTouchEnd, { passive: true });
  body.addEventListener('pointerdown', onPointerDown, { passive: true });
  window.addEventListener('pointerup', onPointerUp, { passive: true });
  window.addEventListener('pointercancel', onPointerUp, { passive: true });
  document.addEventListener('keydown', onKey, { capture: true });
  document.addEventListener('selectionchange', onSelect);
  return () => {
    body.removeEventListener('scroll', b.onScroll);
    body.removeEventListener('wheel', onWheel);
    body.removeEventListener('touchstart', onTouchStart);
    body.removeEventListener('touchmove', onTouchMove);
    body.removeEventListener('touchend', onTouchEnd);
    body.removeEventListener('touchcancel', onTouchEnd);
    body.removeEventListener('pointerdown', onPointerDown);
    window.removeEventListener('pointerup', onPointerUp);
    window.removeEventListener('pointercancel', onPointerUp);
    document.removeEventListener('keydown', onKey, { capture: true });
    document.removeEventListener('selectionchange', onSelect);
  };
}

/** Pictures, dock, viewport and turns watched by one ResizeObserver: every read comes before the one write in a frame. */
export function holdMedia(body: HTMLElement, dock: HTMLElement, away?: (o: { readonly away: boolean; readonly fresh: boolean }) => void): Held {
  const anchors = typeof CSS !== 'undefined' && CSS.supports?.('overflow-anchor', 'auto') === true;
  const sizes = new WeakMap<Element, number | null | typeof PENDING>();
  const size = (m: Element): number | null => (m.getClientRects().length > 0 ? (m as HTMLElement).offsetHeight : null);
  const turnsEl = body.querySelector<HTMLElement>(':scope > [data-kit="turns"]');
  const barEl = body.querySelector<HTMLElement>(':scope > [data-kit="it-bar"]');
  let state: FollowState = atTail(body) ? { ...FOLLOWING, max: maxOf(body) } : { ...PAUSED, max: maxOf(body) };
  const turns = turnsEl ? turnFollower(body, turnsEl, anchors, dock, () => state.following) : undefined;
  let tail = atTail(body);
  let scrollbarDrag = false;
  let lastTop = body.scrollTop;
  let shown = { away: false, fresh: false };
  const tell = (grewAway: boolean): void => {
    const isAway = awayFromEnd(state, body);
    const next = { away: isAway, fresh: isAway && (shown.fresh || grewAway) };
    if (next.away !== shown.away || next.fresh !== shown.fresh) away?.((shown = next));
  };
  const set = (e: FollowEvent): void => {
    const was = state.following;
    state = follow(state, e);
    if (state.following !== was) body.toggleAttribute('data-following', state.following);
  };
  body.toggleAttribute('data-following', state.following);
  const pin = (): void => {
    const before = body.scrollTop;
    body.scrollTop = body.scrollHeight;
    const top = body.scrollTop;
    const max = maxOf(body);
    if (Math.abs(top - before) >= 0.5) set({ type: 'pinned', top, max });
    else lastTop = top;
  };
  const toEnd = (): void => {
    turns?.armNow();
    turns?.clearHand();
    const before = body.scrollTop;
    body.scrollTop = body.scrollHeight;
    const top = body.scrollTop;
    const max = maxOf(body);
    if (Math.abs(top - before) >= 0.5) set({ type: 'end', top, max });
    else {
      lastTop = top;
      set({ type: 'end', max });
    }
    tail = atTail(body);
    turns?.onScroll();
    tell(false);
  };
  const place = (target: number, shouldFollow = false): void => {
    turns?.armNow();
    turns?.clearHand();
    body.scrollTop = target;
    const actual = body.scrollTop;
    const max = maxOf(body);
    lastTop = actual;
    set(shouldFollow ? { type: 'end', max } : { type: 'pause', max });
    tail = atTail(body);
    turns?.onScroll();
    tell(false);
  };
  const held = new Map<Element, number>();
  const ro = new ResizeObserver((entries) => {
    const grew: Grew[] = [];
    const by = { dock: 0, view: 0, turns: 0, bar: 0 };
    for (const { target: m, borderBoxSize } of entries) {
      if (m === dock || m === body || m === turnsEl || m === barEl) {
        const to = borderBoxSize[0]?.blockSize ?? (m as HTMLElement).offsetHeight;
        const from = held.get(m);
        held.set(m, to);
        if (from !== undefined) by[m === dock ? 'dock' : m === body ? 'view' : m === turnsEl ? 'turns' : 'bar'] = to - from;
        continue;
      }
      if (!m.isConnected) {
        ro.unobserve(m);
        continue;
      }
      const from = sizes.get(m);
      const to = size(m);
      sizes.set(m, to);
      if (from !== PENDING && from != null && to != null && Math.abs(to - from) >= 1) grew.push({ top: m.getBoundingClientRect().top, from, to });
    }
    if (!body.isConnected || body.clientHeight === 0) return;
    if (turns?.byHand()) {
      // reader expanded a fold by hand: never yank past it; if the end moved out of view, pause so streaming stays put
      if (state.following && distOf(body) > END_PX) set({ type: 'pause', max: maxOf(body) });
      tell(false);
      return void (tail = atTail(body));
    }
    if (state.following && turns?.armed() === true && !scrollbarDrag) {
      if (grew.length === 0 && Math.abs(by.turns) >= 1 && turns.grew(by.turns)) {
        if (atTail(body)) set({ type: 'pinned', top: body.scrollTop, max: maxOf(body) });
        else {
          lastTop = body.scrollTop;
          set({ type: 'pause', max: maxOf(body) });
          tell(false);
        }
        return void (tail = atTail(body));
      }
      if (distOf(body) > 0.5) {
        pin();
        return void (tail = atTail(body));
      }
    }
    if (grew.length === 0 && Math.abs(by.turns) >= 1) turns?.grew(by.turns);
    if (by.turns >= 1 && !state.following) tell(true);
    const dockEnd = (by.dock !== 0 || by.view !== 0) && dockShift({ scrollHeight: body.scrollHeight, scrollTop: body.scrollTop, clientHeight: body.clientHeight, dock: by.dock, view: by.view }) === 'end';
    if (grew.length === 0 && !dockEnd) return;
    const dy = grew.length === 0 ? 0 : mediaShift({ tail: tail && body.scrollTop > 0, anchors, inset: body.getBoundingClientRect().top + barOf(body), grew });
    if (dy === 'end' || dockEnd) pin();
    else if (Math.abs(dy) >= 1) {
      body.scrollTop += dy;
      lastTop = body.scrollTop;
    }
    tail = atTail(body);
  });
  const watch = (root: ParentNode): void => {
    for (const m of root.querySelectorAll('img, video')) {
      if (sizes.has(m)) continue;
      sizes.set(m, PENDING);
      ro.observe(m);
    }
  };
  watch(body);
  ro.observe(dock);
  ro.observe(body);
  if (turnsEl) ro.observe(turnsEl);
  if (barEl) ro.observe(barEl);
  const mo = new MutationObserver((records) => {
    watch(body);
    turns?.changed(records);
  });
  mo.observe(body, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-live'] });
  const unbind = bindScrollInputs(body, turnsEl, {
    isFollowing: () => state.following,
    onUp: () => {
      set({ type: 'up' });
      tell(false);
    },
    onEnd: toEnd,
    onScroll: () => {
      const top = body.scrollTop;
      set({ type: 'scroll', top, max: maxOf(body), moved: top - lastTop });
      lastTop = top;
      tail = atTail(body);
      turns?.onScroll();
      tell(false);
    },
    onScrollbar: (dragging) => {
      scrollbarDrag = dragging;
      if (!dragging && state.following && distOf(body) > 0.5) pin();
    },
  });
  return {
    stop: () => {
      turns?.stop();
      ro.disconnect();
      mo.disconnect();
      unbind();
    },
    toEnd,
    place,
    following: () => state.following,
  };
}

/** F-page-44 (v1 turns.js renderBody `follow` / `place` / `land`): a turn landing while you read doesn't move you; at
 *  the end Cards mode takes you to its start, while Chat mode keeps pinning the end. Not while the task opens (its
 *  initial placement is the frame's), and not for a fold the reader opened themselves (`byHand`). */
export function turnFollower(
  body: HTMLElement,
  turnsEl: HTMLElement,
  anchors: boolean,
  dock?: HTMLElement,
  following?: () => boolean,
): {
  onScroll: () => void;
  changed: (r: readonly MutationRecord[]) => void;
  grew: (by: number) => boolean;
  armed: () => boolean;
  armNow: () => void;
  byHand: () => boolean;
  clearHand: () => void;
  stop: () => void;
} {
  let armed = false;
  let frame: number | undefined;
  let landed: HTMLElement | undefined;
  let byHand = false;
  let atEnd = atTail(body);
  let reading: { el: HTMLElement; top: number } | undefined;
  const all = (): HTMLElement[] => [...turnsEl.querySelectorAll<HTMLElement>('[data-turn]')];
  const readLine = (): void => {
    if (anchors) return;
    const top = body.getBoundingClientRect().top;
    const el = all().find((t) => t.getBoundingClientRect().bottom > top + 1);
    reading = el ? { el, top: el.getBoundingClientRect().top - top } : undefined;
  };
  const arm = (): void => {
    if (armed || frame !== undefined || !turnsEl.querySelector('[data-turn]')) return;
    frame = requestAnimationFrame(() => {
      frame = undefined;
      armed = true;
      readLine();
    });
  };
  let clear: number | undefined;
  const clearHand = (): void => {
    if (clear !== undefined) cancelAnimationFrame(clear);
    clear = undefined;
    byHand = false;
  };
  const hand = (e: Event): void => {
    if (!isInteractiveIn(e.target, turnsEl)) return;
    byHand = true;
    if (clear !== undefined) cancelAnimationFrame(clear);
    clear = requestAnimationFrame(() => {
      clear = requestAnimationFrame(() => {
        clear = undefined;
        byHand = false;
      });
    });
  };
  body.addEventListener('click', hand, { capture: true, passive: true });
  body.addEventListener('keydown', hand, { capture: true });
  arm();
  return {
    onScroll: () => {
      atEnd = atTail(body);
      if (armed) readLine();
    },
    changed: (records) => {
      arm();
      if (!armed) return;
      const turns = all();
      const last = turns[turns.length - 1];
      if (!last) return;
      for (const r of records) {
        if (r.type === 'childList' && [...r.addedNodes].some((n) => n === last || (n instanceof HTMLElement && n.contains(last)))) landed = last;
        if (r.type === 'attributes' && r.target === last && !last.hasAttribute('data-live')) landed = last;
      }
    },
    grew: (by) => {
      if (!armed || byHand || (by < 0 && !landed)) {
        landed = undefined;
        return false;
      }
      const top = body.getBoundingClientRect().top;
      const tail = following ? following() : by > 0 ? atTail({ scrollHeight: body.scrollHeight - by, scrollTop: body.scrollTop, clientHeight: body.clientHeight }) : atEnd;
      const was = reading?.el.isConnected ? reading : undefined;
      const to = landed?.isConnected ? landed : undefined;
      landed = undefined;
      const chat = body.hasAttribute('data-chat');
      const room = chat ? body.clientHeight - barOf(body) - (dock?.offsetHeight ?? 0) : 0;
      const d = turnsShift({ tail, anchors, landed: !!to, chat, grew: by, room, ...(was && { held: { from: was.top, to: was.el.getBoundingClientRect().top - top } }) });
      if (d === 'land' && to) body.scrollTop += to.getBoundingClientRect().top - top - barOf(body);
      else if (d === 'end') body.scrollTop = body.scrollHeight;
      else if (typeof d === 'number' && Math.abs(d) >= 1) body.scrollTop += d;
      else return false;
      atEnd = atTail(body);
      readLine();
      return true;
    },
    armed: () => armed,
    armNow: () => {
      if (frame !== undefined) cancelAnimationFrame(frame);
      frame = undefined;
      armed = true;
      readLine();
    },
    byHand: () => byHand,
    clearHand,
    stop: () => {
      if (frame !== undefined) cancelAnimationFrame(frame);
      if (clear !== undefined) cancelAnimationFrame(clear);
      body.removeEventListener('click', hand, { capture: true });
      body.removeEventListener('keydown', hand, { capture: true });
    },
  };
}
