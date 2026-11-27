// Where the open task's reader is (TaskView.tsx): each task keeps its place (v1 split.js keepPlace/backToPlace,
// F-task-17), read as the reader scrolls and written when it closes or the page hides, put back once on opening it;
// its Transcript view with it; the turns folded behind the ledger (ADR-0068) and the view sent to the end on a
// follow-up.
import { createEffect, createMemo, createSignal, onCleanup, untrack, type Accessor } from 'solid-js';
import type { Json, EntityKey } from '@wisp/client';
import { foldedFrom } from './rows-view.ts';
import { keep, placeOf, placesLoaded, placesStore, scrollFor, type Place } from './place.ts';
import type { TaskScroll } from '@wisp/ui';
import type { useShell } from '../context.tsx';
import type { useChatSettings } from './chat-settings.ts';
import type { TurnRow } from './task-rows.ts';

type Shell = ReturnType<typeof useShell>;

/** What place keeping reads of the page: the task, its turns, and the body and scroll control its refs hold. */
export interface TaskPlaceCtx {
  readonly shell: Shell;
  readonly key: EntityKey;
  /** the page's own Back, when it has one */
  readonly onBack: (() => void) | undefined;
  readonly chat: ReturnType<typeof useChatSettings>;
  readonly built: Accessor<boolean>;
  readonly turns: () => readonly TurnRow[];
  readonly bodyEl: () => HTMLElement | undefined;
  readonly scrollCtrl: () => TaskScroll | undefined;
}

export function useTaskPlace(c: TaskPlaceCtx) {
  const { shell, key, chat, built, turns, bodyEl, scrollCtrl } = c;
let appliedPlace = false;
let userScrolledBeforePlace = false;
const places = placesStore(shell.client);
const turnTops = (el: HTMLElement): { id: string; top: number }[] => {
  const at = el.getBoundingClientRect().top - el.scrollTop;
  return [...el.querySelectorAll<HTMLElement>('[data-turn]')].map((t) => ({ id: t.dataset['turn'] ?? '', top: t.getBoundingClientRect().top - at }));
};
// kept by the page's own Back just before it leaves: read while the layout is still the frame's (free), where the
// close's read came after the stage had flipped and forced a style and layout of the half-changed page (F-phone-8).
// Back also lets go of the focus first (it goes to the page, as when the button is removed): removing the focused
// button with the page forced another style recalc of the list coming back
let kept = false;
// the place is read as the reader scrolls (once a frame, while the layout is the frame's own), so keeping it on the
// way out reads nothing: read in the close it forced a layout of the half-changed page, 7.5 ms of a Mark done
// (the flicker gate's profile). Never scrolled since it opened: the kept place stands as it was.
let placeSeen: { place: Place | undefined } | undefined;
let reading = 0;
const currentPlace = (el: HTMLElement | undefined, isChat: boolean, tx: boolean): Place | undefined => {
  if (!el || !el.isConnected || el.checkVisibility?.() === false || !el.querySelector('[data-turn]')) {
    const prev = placeSeen?.place ?? (untrack(places.value) as unknown as Readonly<Record<string, Place>> | undefined)?.[key];
    if (!prev) return tx ? { end: isChat, dy: 0, top: 0, transcript: true } : undefined;
    if (tx) return { ...prev, transcript: true };
    const { transcript: _drop, ...rest } = prev;
    return rest.end || rest.turn !== undefined || rest.top > 0 ? rest : undefined;
  }
  return placeOf(el, turnTops(el), { following: scrollCtrl()?.following() === true, chat: isChat, transcript: tx });
};
const notePlace = (el: HTMLElement, isChat: boolean): void => {
  if (reading) return;
  reading = requestAnimationFrame(() => {
    reading = 0;
    // a page already hidden (a phone's Back through history hides the main pane first) has no place to read: every
    // turn would measure 0 and the kept place would become the top
    if (!el.isConnected || el.checkVisibility?.() === false || !el.querySelector('[data-turn]')) return;
    placeSeen = { place: placeOf(el, turnTops(el), { following: scrollCtrl()?.following() === true, chat: isChat, transcript: untrack(stepsShown) }) };
  });
};
onCleanup(() => cancelAnimationFrame(reading));
const keepPlace = (): void => {
  if (kept || !placeSeen) return;
  const p = placeSeen.place;
  places.set((cur) => keep(cur as unknown as Record<string, Place>, key, p) as unknown as Json);
};
onCleanup(keepPlace);
const back = c.onBack;
const leave =
  back &&
  ((): void => {
    keepPlace();
    kept = true;
    const focused = document.activeElement;
    if (focused instanceof HTMLElement && bodyEl()?.parentElement?.contains(focused)) focused.blur();
    back();
  });
const onHide = (): void => {
  if (document.visibilityState === 'hidden') keepPlace();
};
addEventListener('pagehide', keepPlace);
document.addEventListener('visibilitychange', onHide);
onCleanup(() => {
  removeEventListener('pagehide', keepPlace);
  document.removeEventListener('visibilitychange', onHide);
});
const initialPlace = (untrack(places.value) as unknown as Readonly<Record<string, Place>> | undefined)?.[key];
const [stepsOpen, setStepsOpen] = createSignal(initialPlace?.transcript === true);
let stepsTouched = false;
if (!placesLoaded(places)) {
  void places.ready.then(() => {
    if (stepsTouched) return;
    const saved = (untrack(places.value) as unknown as Readonly<Record<string, Place>> | undefined)?.[key];
    if (saved?.transcript !== undefined) setStepsOpen(saved.transcript);
  });
}
const setTaskTranscript = (next: boolean): void => {
  stepsTouched = true;
  const wasFollowing = scrollCtrl()?.following() === true;
  setStepsOpen(next);
  kept = false;
  if (wasFollowing && scrollCtrl()) scrollCtrl()?.toEnd();
  const p = currentPlace(bodyEl(), untrack(chat.chatMode), next);
  placeSeen = { place: p };
  places.set((cur) => keep(cur as unknown as Record<string, Place>, key, p) as unknown as Json);
};
const showStream = (): boolean => chat.chatMode() || stepsOpen();
const stepsShown = (): boolean => stepsOpen();
// the latest turn shows whole; the earlier ones are the ledger's (ADR-0068). Once the reader has left the end, the
// first turn shown whole stays whole as turns land (F-page-44): what they read doesn't fold into a row under them
const [keepFrom, setKeepFrom] = createSignal<string | undefined>(undefined);
// In Chat mode every turn stays visible as a continuous conversation instead of folding behind the Ledger.
const fold = createMemo(() => foldedFrom(turns(), chat.chatMode(), keepFrom(), 1));
const keepShown = (el: HTMLElement): void => {
  if (keepFrom() === undefined && el.scrollHeight - el.scrollTop - el.clientHeight >= 2) setKeepFrom(fold().shown[0]?.id);
};
// a follow-up sent goes to the end: immediately re-engages hold-view's follow state (clearing any byHand fold flag)
// so the optimistic turn and streamed output stay pinned from the very first frame
const toEnd = (): void => {
  setKeepFrom(undefined);
  if (scrollCtrl()) {
    scrollCtrl()?.toEnd();
    requestAnimationFrame(() => {
      if (scrollCtrl()?.following()) scrollCtrl()?.toEnd();
    });
  } else {
    requestAnimationFrame(() => {
      const el = bodyEl();
      if (el) el.scrollTop = el.scrollHeight;
    });
  }
};
let placed = false;
createEffect(() => {
  if (placed || !built() || turns().length === 0) return;
  placed = true;
  const isChat = untrack(chat.chatMode);
  const apply = (): void => {
    appliedPlace = true;
    if (userScrolledBeforePlace) return;
    const el = bodyEl();
    if (!el || !el.isConnected) return;
    const p = (places.value() as unknown as Readonly<Record<string, Place>> | undefined)?.[key];
    if (p) {
      if (p.end || (isChat && p.turn === undefined && p.top <= 1)) {
        if (scrollCtrl()) scrollCtrl()?.toEnd();
        else el.scrollTop = el.scrollHeight;
      } else {
        const top = scrollFor(p, el, turnTops(el));
        if (scrollCtrl()) scrollCtrl()?.place(top, false);
        else el.scrollTop = top;
      }
    } else if (isChat) {
      // Chat mode with no saved reading position opens at the bottom, pinned & following
      if (scrollCtrl()) scrollCtrl()?.toEnd();
      else el.scrollTop = el.scrollHeight;
    } else {
      // Cards mode opens at top; follow if the whole page fits at the end
      scrollCtrl()?.place(el.scrollTop, el.scrollHeight - el.scrollTop - el.clientHeight <= 2);
    }
    // record the initial placed state so closing a Chat task before scrolling never drops it back to undefined/top
    placeSeen = { place: placeOf(el, turnTops(el), { following: scrollCtrl()?.following() === true, chat: isChat, transcript: untrack(stepsShown) }) };
    keepShown(el);
  };
  // once places finished loading (every task switch after boot), place in a microtask before the turns' frame paints
  // so Chat mode lands at the bottom with zero flash of turn 1; on hard reload wait for persisted places first
  if (placesLoaded(places)) queueMicrotask(apply);
  else void places.ready.then(() => requestAnimationFrame(apply));
});
const onBodyScroll = (e: Event): void => {
  kept = false;
  if (built() && !appliedPlace) userScrolledBeforePlace = true;
  const el = e.currentTarget as HTMLElement;
  keepShown(el);
  notePlace(el, untrack(chat.chatMode));
};
  return { leave, stepsOpen, setTaskTranscript, showStream, stepsShown, fold, toEnd, onBodyScroll };
}
