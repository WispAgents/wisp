// F-page-44 (v1 turns.js renderBody follow / place / land) + holdMedia Chat-mode auto-scroll integration tests:
// a turn landing while you read keeps the view put; at the end Cards goes to its start and Chat stays pinned;
// wheel/touch/scrollbar-drag/fold/toEnd/place all coordinate through one follow state machine.
import { afterEach, expect, test, vi } from 'vitest';
import { holdMedia, turnFollower } from '../src/app/hold-view.ts';

const frames: FrameRequestCallback[] = [];
const flush = (): void => {
  for (const f of frames.splice(0)) f(0);
};
afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
  frames.length = 0;
});

function page(o: { scrollHeight: number; clientHeight: number; scrollTop: number; chat?: boolean }) {
  vi.stubGlobal('requestAnimationFrame', (f: FrameRequestCallback) => frames.push(f));
  vi.stubGlobal('cancelAnimationFrame', () => {});
  const body = document.createElement('div');
  if (o.chat) body.setAttribute('data-chat', '');
  const turns = document.createElement('div');
  turns.setAttribute('data-kit', 'turns');
  const dock = document.createElement('div');
  dock.setAttribute('data-kit', 'it-dock');
  body.append(turns, dock);
  document.body.append(body);
  const m = { scrollHeight: o.scrollHeight, clientHeight: o.clientHeight, scrollTop: o.scrollTop };
  Object.defineProperty(body, 'scrollHeight', { get: () => m.scrollHeight });
  Object.defineProperty(body, 'clientHeight', { get: () => m.clientHeight });
  Object.defineProperty(body, 'scrollTop', {
    get: () => m.scrollTop,
    set: (v: number) => {
      m.scrollTop = Math.max(0, Math.min(Math.max(0, m.scrollHeight - m.clientHeight), v));
    },
  });
  body.getBoundingClientRect = () => ({ left: 0, right: 800, top: 0, bottom: m.clientHeight, width: 800, height: m.clientHeight }) as DOMRect;
  const add = (id: string, top: number): HTMLElement => {
    const t = document.createElement('article');
    t.dataset['turn'] = id;
    t.getBoundingClientRect = () => ({ top, bottom: top + 100 }) as DOMRect;
    turns.append(t);
    return t;
  };
  return { body, turns, dock, m, add };
}

test('a_turn_landing_at_the_end_shows_from_its_start_and_one_landing_while_you_read_moves_nothing', () => {
  const p = page({ scrollHeight: 1000, clientHeight: 500, scrollTop: 500 });
  p.add('t1', -400);
  const f = turnFollower(p.body, p.turns, true);
  // opening: not armed until a frame after the first turns are in
  expect(f.grew(300)).toBe(false);
  flush();
  // at the end before it grew (1300 - 300 = 1000 high, 500 down, 500 shown), a new turn lands
  p.m.scrollHeight = 1300;
  const t2 = p.add('t2', 120);
  f.changed([{ type: 'childList', addedNodes: [t2] as unknown as NodeList, target: p.turns } as unknown as MutationRecord]);
  expect(f.grew(300)).toBe(true);
  expect(p.m.scrollTop).toBe(620); // t2's top at the view's top
  // further up: Chrome's anchoring holds it, nothing to do
  p.m.scrollTop = 100;
  const t3 = p.add('t3', 900);
  f.changed([{ type: 'childList', addedNodes: [t3] as unknown as NodeList, target: p.turns } as unknown as MutationRecord]);
  p.m.scrollHeight = 1600;
  expect(f.grew(300)).toBe(false);
  expect(p.m.scrollTop).toBe(100);
  f.stop();
});

test('what_the_reader_opened_themselves_never_moves_the_view_plain_clicks_never_pause_and_webkit_holds_the_turn_being_read', () => {
  const p = page({ scrollHeight: 1000, clientHeight: 500, scrollTop: 500 });
  const t1 = p.add('t1', 0);
  const foldBtn = document.createElement('button');
  const plainText = document.createElement('p');
  t1.append(foldBtn, plainText);
  const f = turnFollower(p.body, p.turns, false);
  flush();
  // clicking plain text (focusing window / clearing selection) does NOT suppress streaming follow
  plainText.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  expect(f.byHand()).toBe(false);
  // clicking a fold button inside a turn suppresses growth for 2 frames so the fold stays in view
  foldBtn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  p.m.scrollHeight = 1400;
  expect(f.grew(400)).toBe(false);
  flush();
  flush(); // two frames on, the flag is gone
  // a key in the reply box (outside the turns) is not the reader changing a turn
  p.body.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'a' }));
  p.m.scrollTop = 900; // at the end of 1400
  p.m.scrollHeight = 1500;
  expect(f.grew(100)).toBe(true);
  expect(p.m.scrollTop).toBe(1000);
  // WebKit, reading further up: what pushed the turn under the reading line down is undone
  p.m.scrollTop = 100;
  f.onScroll(); // t1 is under the reading line at 0
  t1.getBoundingClientRect = () => ({ top: 60, bottom: 160 }) as DOMRect;
  p.m.scrollHeight = 1460;
  expect(f.grew(60)).toBe(true);
  expect(p.m.scrollTop).toBe(160);
  f.stop();
});

test('holdMedia_chat_mode_follows_streaming_ignores_horizontal_and_nonscrollable_wheels_and_resumes_on_toEnd_or_scroll_down', () => {
  let roCb: ResizeObserverCallback | undefined;
  class FakeRO {
    constructor(cb: ResizeObserverCallback) {
      roCb = cb;
    }
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  vi.stubGlobal('ResizeObserver', FakeRO);
  const emitResize = (target: Element, blockSize: number): void => {
    roCb?.([{ target, borderBoxSize: [{ blockSize }] } as unknown as ResizeObserverEntry], {} as ResizeObserver);
  };

  const p = page({ scrollHeight: 500, clientHeight: 500, scrollTop: 0, chat: true });
  p.add('t1', 0);
  const awayStates: Array<{ away: boolean; fresh: boolean }> = [];
  const held = holdMedia(p.body, p.dock, (o) => awayStates.push(o));
  // initial measurements
  emitResize(p.turns, 400);
  emitResize(p.body, 500);
  emitResize(p.dock, 80);

  // wheel up while page doesn't even scroll (scrollTop === 0) must NOT kill following!
  p.body.dispatchEvent(new WheelEvent('wheel', { deltaY: -20 }));
  expect(held.following()).toBe(true);

  // toEnd() arms immediately (e.g. Chat mode initial landing / follow-up) and pins as content overflows
  p.m.scrollHeight = 1500;
  held.toEnd();
  expect(held.following()).toBe(true);
  expect(p.m.scrollTop).toBe(1000);

  // horizontal trackpad swipe with tiny negative deltaY must NOT pause following
  p.body.dispatchEvent(new WheelEvent('wheel', { deltaX: 30, deltaY: -2 }));
  expect(held.following()).toBe(true);

  // streaming chunk grows turns -> ResizeObserver pins straight to new bottom in same frame
  p.m.scrollHeight = 1700;
  emitResize(p.turns, 1600);
  expect(p.m.scrollTop).toBe(1200);
  expect(held.following()).toBe(true);

  // user scrolls up via wheel -> pauses immediately; streaming chunk does NOT yank back and lights fresh dot
  p.body.dispatchEvent(new WheelEvent('wheel', { deltaY: -10 }));
  p.m.scrollTop = 900;
  p.body.dispatchEvent(new Event('scroll'));
  expect(held.following()).toBe(false);
  expect(awayStates.at(-1)).toEqual({ away: true, fresh: false });

  p.m.scrollHeight = 1900;
  emitResize(p.turns, 1800);
  expect(p.m.scrollTop).toBe(900);
  expect(awayStates.at(-1)).toEqual({ away: true, fresh: true });

  // clicking a reply button inside turns (sets byHand) then calling toEnd() clears byHand and re-pins immediately
  const replyBtn = document.createElement('button');
  p.turns.append(replyBtn);
  replyBtn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
  held.toEnd();
  expect(held.following()).toBe(true);
  expect(p.m.scrollTop).toBe(1400);
  p.m.scrollHeight = 2000;
  emitResize(p.turns, 1900);
  expect(p.m.scrollTop).toBe(1500);

  // place(top, false) restores a saved mid-page reading position, paused, with Jump to latest available
  held.place(300, false);
  expect(held.following()).toBe(false);
  expect(p.m.scrollTop).toBe(300);
  expect(awayStates.at(-1)).toEqual({ away: true, fresh: false });

  // wheeling DOWN at the bottom when paused re-engages following even if scrollTop didn't move
  held.place(1500, false);
  expect(held.following()).toBe(false);
  p.body.dispatchEvent(new WheelEvent('wheel', { deltaY: 20 }));
  expect(held.following()).toBe(true);

  held.stop();
});

test('holdMedia_keeps_following_when_already_at_bottom_across_async_scroll_events_and_early_growth_before_rAF_arming', () => {
  let roCb: ResizeObserverCallback | undefined;
  class FakeRO {
    constructor(cb: ResizeObserverCallback) {
      roCb = cb;
    }
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  vi.stubGlobal('ResizeObserver', FakeRO);
  const emitResize = (target: Element, blockSize: number): void => {
    roCb?.([{ target, borderBoxSize: [{ blockSize }] } as unknown as ResizeObserverEntry], {} as ResizeObserver);
  };

  // Start already at the bottom (scrollTop=0, scrollHeight=500, clientHeight=500), before rAF arming has run
  const p = page({ scrollHeight: 500, clientHeight: 500, scrollTop: 0, chat: true });
  p.add('t1', 0);
  const held = holdMedia(p.body, p.dock);
  expect(held.following()).toBe(true);

  // Content grows BEFORE flush() (before turnFollower's initial rAF callback fires):
  // holdMedia must still pin to the new bottom immediately!
  p.m.scrollHeight = 900;
  emitResize(p.turns, 800);
  expect(p.m.scrollTop).toBe(400);
  expect(held.following()).toBe(true);

  // Now another streaming chunk lands BEFORE the browser fires the async scroll event for scrollTop=400:
  // If lastTop wasn't updated when pin() wrote scrollTop=400, the delayed scroll event after a slight layout shift
  // or browser clamp could compute a negative `moved` and kill following. Verify following survives!
  p.m.scrollHeight = 880; // routine steps collapsed or layout settled by -20px, browser clamps scrollTop to 380
  p.m.scrollTop = 380;
  p.body.dispatchEvent(new Event('scroll'));
  expect(held.following()).toBe(true);

  // Next streaming chunk grows the page to 1200 -> scrolls straight to 700 because we never lost following
  p.m.scrollHeight = 1200;
  emitResize(p.turns, 1100);
  expect(p.m.scrollTop).toBe(700);
  expect(held.following()).toBe(true);

  held.stop();
});
