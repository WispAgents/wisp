/// <reference types="vite/client" />
// ADR-0065 build step 7: the kit components that carry a gesture (design/mobile.md §A.4–§A.6). Under a phone look
// each marks its element (data-gesture, data-twin: qa/platform.mjs's twin rule), runs the gesture, ticks through the
// GestureHost, and gives a screen reader the same thing as custom actions. The browser at any width gets none of it:
// its HTML is byte for byte what it was (the step 3–4 goldens), the gesture props passed or not.
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { createSignal, type JSX } from 'solid-js';
import rowsGoldenRaw from './goldens/platform-browser-rows.json?raw';
import { mount } from './helpers.tsx';
import { binders } from '../src/gestures/binders-part.ts';
import { rowScenes } from './platform-scenes-rows.tsx';
import {
  GestureHostProvider,
  ListRow,
  PAGE_SWIPE_LINE,
  PULL_LINE,
  PULL_RESIST,
  PlatformLookProvider,
  Swipe,
  Text,
  TopAppBar,
  VirtualList,
  useSidePanelSwipe,
  type GestureAction,
  type GestureHost,
  type Haptic,
  type Look,
} from '../src/index.ts';

const ROWS_GOLDEN = JSON.parse(rowsGoldenRaw) as Record<string, string>;
const W = 412;
const noop = (): void => {};
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);

interface FakeHost extends GestureHost {
  readonly ticks: Haptic[];
  readonly given: Map<Element, readonly GestureAction[]>;
}
function fakeHost(): FakeHost {
  const ticks: Haptic[] = [];
  const given = new Map<Element, readonly GestureAction[]>();
  return {
    ticks,
    given,
    haptic: (k) => void ticks.push(k),
    actions: (el, list) => {
      given.set(el, [...(given.get(el) ?? []), ...list]);
      return () => given.delete(el);
    },
  };
}
const on = (look: Look, h: GestureHost, view: () => JSX.Element): HTMLElement =>
  mount(() => (
    <PlatformLookProvider look={() => look}>
      <GestureHostProvider host={h}>{view()}</GestureHostProvider>
    </PlatformLookProvider>
  ));
const labels = (h: FakeHost, el: Element | null): string[] => (el ? (h.given.get(el) ?? []).map((a) => a.label) : []);

function pe(el: Element, type: string, x: number, y = 100): void {
  el.dispatchEvent(new PointerEvent(type, { bubbles: true, cancelable: true, clientX: x, clientY: y, pointerType: 'touch', isPrimary: true, pointerId: 1 } as PointerEventInit));
}
function te(el: Element, type: string, x: number, y: number): void {
  const e = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(e, { touches: { value: type === 'touchend' ? [] : [{ clientX: x, clientY: y }] }, changedTouches: { value: [{ clientX: x, clientY: y }] } });
  el.dispatchEvent(e);
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

// the phone apps load the touch binders with their look (platform/styles.ts) before the first screen draws
beforeAll(() => binders.load().then(() => undefined));

describe('the browser gets no gestures', () => {
  it('a_browser_render_with_every_gesture_prop_is_byte_identical_to_before', () => {
    // the step-4 goldens, with the gesture props a phone caller passes too
    const extra = (kind: string): Record<string, unknown> => (kind === 'virtual' ? { refresh: { onRefresh: noop, refreshing: false, label: 'Reconnecting' } } : {});
    for (const [name, view] of rowScenes(extra)) {
      const host = mount(view);
      expect(host.innerHTML, name).toBe(ROWS_GOLDEN[name]);
      host.remove();
    }
    const swipe = mount(() => (
      <Swipe right={{ label: 'Mark done', icon: 'done', tone: 'good' }} left={{ label: 'Save for later', icon: 'bookmark', tone: 'accent' }} onSwipe={noop} onLongPress={noop}>
        <ListRow title="Row" />
      </Swipe>
    ));
    expect(swipe.innerHTML).toBe(ROWS_GOLDEN['swipe']);
    const plain = mount(() => <TopAppBar kind="page" title="Fix it" back={{ label: 'Tasks', onBack: noop }} />).innerHTML;
    const withSwipe = mount(() => <TopAppBar kind="page" title="Fix it" back={{ label: 'Tasks', onBack: noop }} swipe={{ onNext: noop, onPrevious: noop }} />).innerHTML;
    expect(withSwipe).toBe(plain);
    expect(document.querySelector('[data-gesture], [data-twin]')).toBeNull();
  });

  it('a_browser_row_does_not_open_a_menu_on_a_long_press_or_swipe_from_a_page', () => {
    vi.useFakeTimers();
    vi.stubGlobal('innerWidth', W);
    const onLongPress = vi.fn();
    const onPanel = vi.fn();
    function Body(): JSX.Element {
      const ref = useSidePanelSwipe(onPanel);
      return <div ref={ref} id="body">body</div>;
    }
    mount(() => (
      <>
        <Swipe onSwipe={noop} onLongPress={onLongPress}>
          <ListRow title="Row" />
        </Swipe>
        <Body />
      </>
    ));
    const row = q('[data-kit="swipe"]');
    if (!row) throw new Error('no row');
    pe(row, 'pointerdown', 200);
    vi.advanceTimersByTime(800);
    const body = q('#body');
    if (!body) throw new Error('no body');
    pe(body, 'pointerdown', 300);
    pe(body, 'pointermove', 250);
    pe(body, 'pointermove', 100);
    pe(body, 'pointerup', 100);
    expect(onLongPress).not.toHaveBeenCalled();
    expect(onPanel).not.toHaveBeenCalled();
  });
});

describe('a row (Swipe) in the phone apps', () => {
  const rect = (): void => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({ width: W, height: 72, top: 0, left: 0, right: W, bottom: 72, x: 0, y: 0, toJSON: () => ({}) } as DOMRect);
  };

  for (const look of ['android', 'ios'] as const) {
    it(`${look}_a_row_names_its_gestures_and_twins_and_gives_them_to_a_screen_reader`, () => {
      const h = fakeHost();
      const onSwipe = vi.fn();
      const onLongPress = vi.fn();
      on(look, h, () => (
        <Swipe right={{ label: 'Mark done', icon: 'done', tone: 'good' }} left={{ label: 'Save for later', icon: 'bookmark', tone: 'accent' }} onSwipe={onSwipe} onLongPress={onLongPress}>
          <ListRow title="Row" />
        </Swipe>
      ));
      const row = q('[data-kit="swipe"]');
      expect(row?.getAttribute('data-gesture')).toBe('row.swipe row.long-press');
      expect(row?.getAttribute('data-twin')).toBe('ui.app-bar.actions');
      expect(labels(h, row)).toEqual(['Mark done', 'Save for later', 'Actions']);
      // TalkBack / VoiceOver run them without the gesture
      h.given.get(row as Element)?.[0]?.run();
      expect(onSwipe).toHaveBeenCalledWith('right');
      h.given.get(row as Element)?.[2]?.run();
      expect(onLongPress).toHaveBeenCalledTimes(1);
    });
  }

  it('android_a_long_press_on_a_row_opens_its_actions_with_a_tap', () => {
    vi.useFakeTimers();
    vi.stubGlobal('innerWidth', W);
    rect();
    const h = fakeHost();
    const onLongPress = vi.fn();
    const onSwipe = vi.fn();
    on('android', h, () => (
      <Swipe right={{ label: 'Mark done', icon: 'done', tone: 'good' }} onSwipe={onSwipe} onLongPress={onLongPress}>
        <ListRow title="Row" />
      </Swipe>
    ));
    const row = q('[data-kit="swipe"]');
    if (!row) throw new Error('no row');
    pe(row, 'pointerdown', 200);
    vi.advanceTimersByTime(500);
    expect(onLongPress).toHaveBeenCalledTimes(1);
    expect(h.ticks).toEqual(['tap']);
    // a drag after the menu opened is not a swipe
    pe(row, 'pointermove', 260);
    pe(row, 'pointermove', 380);
    expect(row.getAttribute('data-phase')).toBe('idle');
  });

  it('a_swipe_crossing_the_line_ticks_select_through_the_host', () => {
    vi.stubGlobal('innerWidth', W);
    rect();
    const h = fakeHost();
    on('android', h, () => (
      <Swipe right={{ label: 'Mark done', icon: 'done', tone: 'good' }} onSwipe={noop}>
        <ListRow title="Row" />
      </Swipe>
    ));
    const row = q('[data-kit="swipe"]');
    if (!row) throw new Error('no row');
    pe(row, 'pointerdown', 60);
    for (let i = 1; i <= 10; i++) pe(row, 'pointermove', 60 + i * 25);
    expect(row.hasAttribute('data-armed')).toBe(true);
    expect(h.ticks).toEqual(['select']);
  });

  it('a_swipe_in_a_chip_row_inside_a_row_scrolls_the_chips', () => {
    vi.stubGlobal('innerWidth', W);
    rect();
    const onSwipe = vi.fn();
    on('android', fakeHost(), () => (
      <Swipe right={{ label: 'Mark done', icon: 'done', tone: 'good' }} onSwipe={onSwipe}>
        <ListRow title="Row" sub={<span data-scroll-x="" id="chips"><Text>wisp</Text></span>} />
      </Swipe>
    ));
    const chip = q('#chips span');
    const row = q('[data-kit="swipe"]');
    if (!chip || !row) throw new Error('no chips');
    pe(chip, 'pointerdown', 60);
    for (let i = 1; i <= 10; i++) pe(chip, 'pointermove', 60 + i * 25);
    expect(row.getAttribute('data-phase')).toBe('idle');
  });
});

describe('pull to refresh on a list', () => {
  it('android_pulling_the_list_past_the_line_reconnects_and_draws_the_pull', () => {
    vi.stubGlobal('innerWidth', W);
    const h = fakeHost();
    const onRefresh = vi.fn();
    on('android', h, () => (
      <VirtualList aria-label="Tasks" items={['a', 'b']} key={(s) => s} viewport={400} refresh={{ onRefresh, refreshing: false, label: 'Reconnecting' }}>
        {(s) => <Text>{s()}</Text>}
      </VirtualList>
    ));
    const list = q('[data-kit="virtual-list"]');
    if (!list) throw new Error('no list');
    expect(list.getAttribute('data-gesture')).toBe('list.pull');
    expect(list.getAttribute('data-twin')).toBe('ui.app-bar.actions');
    expect(labels(h, list)).toEqual(['Reconnect']);
    const past = 100 + PULL_LINE / PULL_RESIST + 30;
    te(list, 'touchstart', 200, 100);
    te(list, 'touchmove', 200, 130);
    te(list, 'touchmove', 200, past);
    expect(q('[data-kit="pull-refresh"]')?.hasAttribute('data-armed')).toBe(true);
    te(list, 'touchend', 200, past);
    expect(onRefresh).toHaveBeenCalledTimes(1);
    expect(h.ticks).toEqual(['select']);
    // let go: the pull is gone until the caller says it's refreshing
    expect(q('[data-kit="pull-refresh"]')).toBeNull();
  });
});

describe('a task page', () => {
  it('ios_swiping_the_head_opens_the_next_and_previous_task', () => {
    vi.stubGlobal('innerWidth', W);
    const h = fakeHost();
    const onNext = vi.fn();
    const onPrevious = vi.fn();
    on('ios', h, () => <TopAppBar kind="page" title="Fix it" back={{ label: 'Tasks', onBack: noop }} swipe={{ onNext, onPrevious }} actions={[{ id: 'more', label: 'Next task', icon: 'chevron', priority: 1, onSelect: noop }]} />);
    const head = q('[data-kit="app-bar"]');
    if (!head) throw new Error('no head');
    expect(head.getAttribute('data-gesture')).toBe('task.head-swipe');
    expect(head.getAttribute('data-twin')).toBe('ui.app-bar.actions');
    // its twin is on screen in the same bar
    expect(q('[data-part~="ui.app-bar.actions"]')).not.toBeNull();
    expect(labels(h, head)).toEqual(['Next task', 'Previous task']);
    const go = (from: number, to: number): void => {
      pe(head, 'pointerdown', from);
      for (let i = 1; i <= 6; i++) pe(head, 'pointermove', from + ((to - from) * i) / 6);
      pe(head, 'pointerup', to);
    };
    go(300, 300 - PAGE_SWIPE_LINE - 10);
    expect(onNext).toHaveBeenCalledTimes(1);
    go(100, 100 + PAGE_SWIPE_LINE + 10);
    expect(onPrevious).toHaveBeenCalledTimes(1);
    // from the left edge: the system's Back, not the previous task
    go(10, 10 + PAGE_SWIPE_LINE + 60);
    expect(onPrevious).toHaveBeenCalledTimes(1);
  });

  it('a_screen_reader_gets_next_task_once_there_is_one', () => {
    const h = fakeHost();
    const [next, setNext] = createSignal<(() => void) | undefined>(undefined);
    on('android', h, () => <TopAppBar kind="page" title="Fix it" swipe={{ onNext: next(), onPrevious: noop }} />);
    const head = q('[data-kit="app-bar"]');
    expect(labels(h, head)).toEqual(['Previous task']);
    setNext(() => noop);
    expect(labels(h, head)).toEqual(['Next task', 'Previous task']);
  });

  it('android_swiping_left_on_the_body_brings_the_side_panel', () => {
    vi.stubGlobal('innerWidth', W);
    const h = fakeHost();
    const onPanel = vi.fn();
    function Body(): JSX.Element {
      const ref = useSidePanelSwipe(onPanel);
      return <div ref={ref} id="body"><Text>the transcript</Text></div>;
    }
    on('android', h, () => <Body />);
    const body = q('#body');
    if (!body) throw new Error('no body');
    expect(body.getAttribute('data-gesture')).toBe('task.body-swipe');
    expect(labels(h, body)).toEqual(['Side panel']);
    pe(body, 'pointerdown', 300);
    for (let i = 1; i <= 6; i++) pe(body, 'pointermove', 300 - i * 30);
    pe(body, 'pointerup', 120);
    expect(onPanel).toHaveBeenCalledTimes(1);
    expect(h.ticks).toEqual(['select']);
    // rightward brings nothing
    pe(body, 'pointerdown', 100);
    for (let i = 1; i <= 6; i++) pe(body, 'pointermove', 100 + i * 30);
    pe(body, 'pointerup', 280);
    expect(onPanel).toHaveBeenCalledTimes(1);
  });
});
