import { afterEach, describe, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import { Each, VirtualList } from '../src/layout/VirtualList.tsx';
import { anchorAt, anchors, atTail, computeRange, offsets, restoreAnchor, rowAt, TAIL_SLACK } from '../src/layout/virtual-math.ts';

/** A small seeded PRNG (mulberry32), so a failing case can be replayed from its seed. */
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const int = (r: () => number, max: number): number => Math.floor(r() * (max + 1));

describe('virtual-math (property tests, 500 seeded cases each)', () => {
  test('offsets are monotonic and end at the sum', () => {
    for (let seed = 1; seed <= 500; seed++) {
      const r = rng(seed);
      const heights = Array.from({ length: int(r, 60) }, () => int(r, 120));
      const o = offsets(heights);
      expect(o.length, `seed ${seed}`).toBe(heights.length + 1);
      for (let i = 1; i < o.length; i++) expect((o[i] ?? 0) >= (o[i - 1] ?? 0), `seed ${seed}`).toBe(true);
      expect(o[o.length - 1]).toBe(heights.reduce((a, b) => a + b, 0));
    }
  });

  test('a range covers the viewport and spacers + rows add up to the total', () => {
    for (let seed = 1; seed <= 500; seed++) {
      const r = rng(seed);
      const heights = Array.from({ length: int(r, 80) }, () => 1 + int(r, 90));
      const o = offsets(heights);
      const total = o[o.length - 1] ?? 0;
      const view = int(r, 700);
      const top = int(r, total + 200) - 50;
      const overscan = int(r, 5);
      const g = computeRange(o, top, view, overscan);
      const rendered = (o[g.end] ?? 0) - (o[g.start] ?? 0);
      expect(g.padTop + rendered + g.padBottom, `seed ${seed}`).toBe(total);
      expect(g.start <= g.end && g.end <= heights.length, `seed ${seed}`).toBe(true);
      const clamped = Math.min(Math.max(0, top), Math.max(0, total - view));
      expect(g.padTop <= clamped, `seed ${seed}: top spacer reaches into the viewport`).toBe(true);
      expect(total - g.padBottom >= Math.min(total, clamped + view), `seed ${seed}: viewport bottom uncovered`).toBe(true);
    }
  });

  test('an anchor restores to the same scrollTop when nothing changed', () => {
    for (let seed = 1; seed <= 500; seed++) {
      const r = rng(seed);
      const o = offsets(Array.from({ length: 1 + int(r, 40) }, () => 1 + int(r, 60)));
      const top = int(r, o[o.length - 1] ?? 0);
      expect(restoreAnchor(o, anchorAt(o, top)), `seed ${seed}`).toBe(top);
    }
  });

  test('a row growing above the anchor moves the restored scrollTop by exactly the growth', () => {
    const before = offsets([50, 50, 50, 50]);
    const a = anchorAt(before, 120); // row 2, 20px in
    expect(a).toEqual({ index: 2, delta: 20 });
    expect(restoreAnchor(offsets([50, 80, 50, 50]), a)).toBe(150);
  });

  test('a_list_at_its_top_stays_at_its_top_when_rows_arrive_above_and_a_transcript_keeps_its_row', () => {
    // the phone's Tasks list: Needs you fills in after the first paint, the list was never scrolled
    expect(anchors(0)).toBe(false);
    expect(anchors(0, 'top')).toBe(false);
    // scrolled into: the row under the top stays put as rows grow above (the rule above)
    expect(anchors(1)).toBe(true);
    // a transcript reading its oldest turn while older ones page in above keeps that turn in place
    expect(anchors(0, 'bottom')).toBe(true);
  });

  test('rowAt, empty lists and the tail slack', () => {
    expect(rowAt(offsets([10, 10, 10]), 15)).toBe(1);
    expect(computeRange(offsets([]), 0, 500, 3)).toEqual({ start: 0, end: 0, padTop: 0, padBottom: 0, total: 0 });
    expect(TAIL_SLACK).toBe(40);
    expect(atTail(960, 500, 1500)).toBe(true);
    expect(atTail(959, 500, 1500)).toBe(false);
  });
});

interface Row {
  readonly id: string;
  readonly text: string;
}

const rows = (n: number): Row[] => Array.from({ length: n }, (_, i) => ({ id: `r${i}`, text: `Row ${i}` }));

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});

function mount(fn: () => ReturnType<typeof VirtualList>): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(fn, host);
  return host;
}

const nodeFor = (host: HTMLElement, id: string): Element | null => host.querySelector(`[data-id="${id}"]`);

describe('VirtualList', () => {
  test('renders its data-part, rows and spacers, not every row', () => {
    const host = mount(() => (
      <VirtualList items={rows(5000)} key={(r) => r.id} estimate={30} viewport={300}>
        {(r) => <span data-id={r().id}>{r().text}</span>}
      </VirtualList>
    ));
    expect(host.querySelector('[data-part="ui.virtual-list"]')).not.toBeNull();
    const count = host.querySelectorAll('[data-kit="virtual-row"]').length;
    expect(count).toBeGreaterThan(5);
    expect(count).toBeLessThan(40);
    const spacers = host.querySelectorAll<HTMLElement>('[data-kit="virtual-spacer"]');
    expect(spacers.length).toBe(2);
    expect(spacers[1]?.style.height).not.toBe('0px');
  });

  test('a reorder moves the same DOM nodes in place', () => {
    const [items, setItems] = createSignal(rows(10));
    const host = mount(() => (
      <VirtualList items={items()} key={(r) => r.id} estimate={30} viewport={600}>
        {(r) => <span data-id={r().id}>{r().text}</span>}
      </VirtualList>
    ));
    const before = items().map((r) => nodeFor(host, r.id));
    expect(before.every((n) => n !== null)).toBe(true);
    setItems([...items()].reverse());
    const after = items().map((r) => nodeFor(host, r.id));
    // Same node per key, now in the new order.
    for (const r of items()) expect(nodeFor(host, r.id)).toBe(before[Number(r.id.slice(1))]);
    const order = [...host.querySelectorAll('[data-id]')].map((n) => n.getAttribute('data-id'));
    expect(order).toEqual(items().map((r) => r.id));
    expect(after.length).toBe(10);
  });

  test('an updated item keeps its node and shows the new value', () => {
    const [items, setItems] = createSignal(rows(3));
    const host = mount(() => (
      <VirtualList items={items()} key={(r) => r.id} viewport={600}>
        {(r) => <span data-id={r().id}>{r().text}</span>}
      </VirtualList>
    ));
    const node = nodeFor(host, 'r1');
    setItems(items().map((r) => (r.id === 'r1' ? { ...r, text: 'Renamed' } : r)));
    expect(nodeFor(host, 'r1')).toBe(node);
    expect(node?.textContent).toBe('Renamed');
  });

  test('passes each row its index in the whole list', () => {
    const host = mount(() => (
      <VirtualList items={rows(4)} key={(r) => r.id} viewport={600}>
        {(r, i) => <span data-id={r().id}>{i()}</span>}
      </VirtualList>
    ));
    expect(nodeFor(host, 'r3')?.textContent).toBe('3');
  });

  test('bottom anchor marks the list as a transcript', () => {
    const host = mount(() => (
      <VirtualList items={rows(50)} key={(r) => r.id} anchor="bottom" viewport={200} part="task.transcript">
        {(r) => <span data-id={r().id}>{r().text}</span>}
      </VirtualList>
    ));
    expect(host.querySelector('[data-part="task.transcript"]')?.getAttribute('data-anchor')).toBe('bottom');
  });
});

describe('Each', () => {
  test('keeps keyed nodes in place on reorder, without an element of its own', () => {
    const [items, setItems] = createSignal(rows(5));
    const host = document.createElement('ul');
    document.body.append(host);
    dispose = render(() => <Each items={items()} key={(r) => r.id}>{(r) => <li data-id={r().id}>{r().text}</li>}</Each>, host);
    const first = nodeFor(host, 'r0');
    setItems([...items()].reverse());
    expect(host.lastElementChild).toBe(first);
    expect(host.children.length).toBe(5);
  });
});

test('a_row_says_when_it_sits_a_list_gap_below_the_one_before_it', () => {
  // the task list: two task rows are 2px apart, a heading and the row after it are not (v1 .f-list gap)
  const items = [{ id: 'h', text: 'Needs you' }, { id: 'a', text: 'A' }, { id: 'b', text: 'B' }];
  const host = mount(() => (
    <VirtualList items={items} key={(r) => r.id} estimate={30} viewport={300} gapBefore={(i) => i > 1}>
      {(r) => <span data-id={r().id}>{r().text}</span>}
    </VirtualList>
  ));
  const gap = (id: string): boolean | undefined => nodeFor(host, id)?.closest('[data-kit="virtual-row"]')?.hasAttribute('data-gap');
  expect([gap('h'), gap('a'), gap('b')]).toEqual([false, false, true]);
});

test('a_scroll_reads_layout_before_it_renders_rows_never_after', () => {
  // F-phone-8: reading scrollTop after the new rows are in the DOM forces a reflow on every scroll frame (60 ms a
  // frame on a 4× slowed phone with 4,000 tasks). A scroll reads its geometry first, then renders.
  const host = mount(() => (
    <VirtualList items={rows(4000)} key={(r) => r.id} estimate={30} viewport={300}>
      {(r) => <span data-id={r().id}>{r().text}</span>}
    </VirtualList>
  ));
  const list = host.querySelector<HTMLElement>('[data-kit="virtual-list"]');
  if (!list) throw new Error('no list');
  const first = (): string | null | undefined => list.querySelector('[data-id]')?.getAttribute('data-id');
  let top = 0;
  const seen: (string | null | undefined)[] = [];
  Object.defineProperty(list, 'scrollTop', {
    configurable: true,
    get: () => (seen.push(first()), top),
    set: (v: number) => void (top = v),
  });
  Object.defineProperty(list, 'clientHeight', { configurable: true, get: () => 300 });
  top = 30 * 2000;
  const before = first();
  list.dispatchEvent(new Event('scroll'));
  expect(first()).not.toBe(before); // the rows at row 2000 are on screen now
  expect(seen.length).toBeGreaterThan(0);
  expect(seen.every((f) => f === before), `scrollTop read after rows changed: ${seen.join(',')}`).toBe(true);
});

/** A ResizeObserver the test drives: `report` delivers entries the way the browser does after a layout. */
class FakeObserver {
  static last: FakeObserver | undefined;
  readonly watched = new Set<Element>();
  private readonly cb: (entries: ResizeObserverEntry[]) => void;
  constructor(cb: (entries: ResizeObserverEntry[]) => void) {
    this.cb = cb;
    FakeObserver.last = this;
  }
  readonly boxes = new Map<Element, string | undefined>();
  observe(el: Element, opts?: ResizeObserverOptions): void {
    this.watched.add(el);
    this.boxes.set(el, opts?.box);
  }
  unobserve(el: Element): void {
    this.watched.delete(el);
  }
  disconnect(): void {
    this.watched.clear();
  }
  report(size: (el: Element) => { width: number; height: number }): void {
    const entries = [...this.watched].map((target) => {
      const { width, height } = size(target);
      return { target, borderBoxSize: [{ inlineSize: width, blockSize: height }], contentRect: { width, height } } as unknown as ResizeObserverEntry;
    });
    this.cb(entries);
  }
}

describe('VirtualList measured by a ResizeObserver', () => {
  afterEach(async () => {
    vi.unstubAllGlobals();
    // a test that delivers observer callbacks itself opens a kit observer pass (core/observer-scope.ts), which ends at
    // the next task as in a browser: wait for it, so the next test starts outside one
    await new Promise<void>((r) => setTimeout(r, 0));
  });

  // F-phone-8: a phone's task page hides the list (display: none / content-visibility: hidden); the observer then
  // reports the list's box and every row at 0 px. Recording that re-planned the window and swapped its rows on every
  // tap, and rebuilt them all on Back.
  test('a_list_taken_off_screen_keeps_its_rows_and_their_heights', () => {
    vi.stubGlobal('ResizeObserver', FakeObserver);
    const host = mount(() => (
      <VirtualList items={rows(200)} key={(r) => r.id} estimate={30} viewport={300} overscan={2}>
        {(r) => <span data-id={r().id}>{r().text}</span>}
      </VirtualList>
    ));
    const ro = FakeObserver.last;
    if (!ro) throw new Error('no observer');
    const box = host.querySelector('[data-kit="virtual-list"]');
    const shown = (): (string | null)[] => [...host.querySelectorAll('[data-id]')].map((n) => n.getAttribute('data-id'));
    const bottom = (): string | undefined => host.querySelectorAll<HTMLElement>('[data-kit="virtual-spacer"]')[1]?.style.height;
    // on screen: the rows measure 30 px, as estimated: nothing moves
    ro.report((el) => (el === box ? { width: 390, height: 300 } : { width: 390, height: 30 }));
    const before = shown();
    const nodes = [...host.querySelectorAll('[data-id]')];
    const pad = bottom();
    // hidden: the box and every row report 0 px
    ro.report(() => ({ width: 0, height: 0 }));
    expect(shown()).toEqual(before);
    expect([...host.querySelectorAll('[data-id]')]).toEqual(nodes);
    expect(bottom()).toBe(pad);
    // shown again at the same sizes: still the same rows, the same nodes
    ro.report((el) => (el === box ? { width: 390, height: 300 } : { width: 390, height: 30 }));
    expect([...host.querySelectorAll('[data-id]')]).toEqual(nodes);
    // while on screen a row that really grew is still recorded: the spacers move
    ro.report((el) => (el === box ? { width: 390, height: 300 } : { width: 390, height: 60 }));
    expect(shown()).not.toEqual(before);
  });

  // QA round 1 #16, round 2 #15: measured rows shortened the list, its (classic, room-taking) scrollbar went away, and
  // every row grew a scrollbar wider in the same pass, which the browser skips at the rows' depth: WebKit's "loop
  // completed with undelivered notifications". The re-plan still lands in that pass (deferring it a frame made the
  // rows above the reader and a streaming tail jump); only the width-only change waits: the rows step out of the
  // observer until the next frame, and the box is watched by its border box, which a scrollbar never changes.
  test('a_scrollbar_change_from_a_replan_never_delays_the_replan_and_leaves_nothing_undelivered', async () => {
    vi.stubGlobal('ResizeObserver', FakeObserver);
    const host = mount(() => (
      <VirtualList items={rows(200)} key={(r) => r.id} estimate={30} viewport={300} overscan={2}>
        {(r) => <span data-id={r().id}>{r().text}</span>}
      </VirtualList>
    ));
    const ro = FakeObserver.last;
    if (!ro) throw new Error('no observer');
    const box = host.querySelector<HTMLElement>('[data-kit="virtual-list"]');
    if (!box) throw new Error('no box');
    expect(ro.boxes.get(box), 'the box by its border box').toBe('border-box');
    const bottom = (): string | undefined => host.querySelectorAll<HTMLElement>('[data-kit="virtual-spacer"]')[1]?.style.height;
    const before = bottom();
    // the scrollbar goes as the re-plan lands (the list's, or one of a scroller around it: the sidebar's): every row is
    // 10px wider once the spacers moved
    const was = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth');
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, get: () => (bottom() === before ? 380 : 390) });
    const rowsWatched = (): number => [...ro.watched].filter((el) => el !== box).length;
    const watchedBefore = rowsWatched();
    expect(watchedBefore).toBeGreaterThan(0);
    // the rows are really 20px: the window re-plans at once, and the scrollbar goes with it
    ro.report((el) => (el === box ? { width: 390, height: 300 } : { width: 380, height: 20 }));
    expect(bottom(), 're-planned in the same pass, not a frame later').not.toBe(before);
    expect(rowsWatched(), 'the rows step out while the width change goes by').toBe(0);
    await new Promise<void>((r) => requestAnimationFrame(() => r()));
    expect(rowsWatched(), 'and are measured again on the next frame').toBeGreaterThan(0);
    if (was) Object.defineProperty(HTMLElement.prototype, 'offsetWidth', was);
  });

  // perf r1 #4: a list from the top read its height as it mounted, mid render, forcing the page's whole first layout
  // (109 ms of the first screen at 4×). It takes it from its observer's first pass, after layout and before paint.
  test('a_list_from_the_top_takes_its_height_from_its_observer_not_a_read_at_mount', () => {
    vi.stubGlobal('ResizeObserver', FakeObserver);
    let reads = 0;
    let h = 0;
    const was = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientHeight');
    Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get(this: HTMLElement) { if (this.dataset['kit'] === 'virtual-list') reads++; return this.dataset['kit'] === 'virtual-list' ? h : 0; } });
    try {
      const host = mount(() => (
        <VirtualList items={rows(400)} key={(r) => r.id} estimate={30} viewport={300} overscan={0}>
          {(r) => <span data-id={r().id}>{r().text}</span>}
        </VirtualList>
      ));
      expect(reads, 'no layout read as it mounts').toBe(0);
      const shown = (): number => host.querySelectorAll('[data-id]').length;
      const before = shown();
      const box = host.querySelector('[data-kit="virtual-list"]');
      h = 600;
      FakeObserver.last?.report((el) => (el === box ? { width: 390, height: 600 } : { width: 390, height: 30 }));
      expect(shown(), 'the observer pass gave it its real height: more rows').toBeGreaterThan(before);
    } finally {
      if (was) Object.defineProperty(HTMLElement.prototype, 'clientHeight', was);
    }
  });

  test('a_replan_that_leaves_the_scrollbar_as_it_was_keeps_every_row_observed', () => {
    vi.stubGlobal('ResizeObserver', FakeObserver);
    const host = mount(() => (
      <VirtualList items={rows(200)} key={(r) => r.id} estimate={30} viewport={300} overscan={2}>
        {(r) => <span data-id={r().id}>{r().text}</span>}
      </VirtualList>
    ));
    const ro = FakeObserver.last;
    const box = host.querySelector<HTMLElement>('[data-kit="virtual-list"]');
    if (!ro || !box) throw new Error('no list');
    const watched = ro.watched.size;
    ro.report((el) => (el === box ? { width: 390, height: 300 } : { width: 390, height: 20 }));
    expect(ro.watched.size).toBeGreaterThanOrEqual(watched);
    expect([...ro.watched].some((el) => el !== box)).toBe(true);
  });

  // QA round 2 desktop #16: Settings → Diagnostics' event list grew as it was measured: the rows its re-plan mounted in
  // the observer's own pass were observed in that pass, where their first size is skipped (the loop error). They're
  // observed on the next frame instead; the re-plan itself still lands in the pass.
  test('rows_a_replan_mounts_during_the_observers_pass_are_observed_next_frame', async () => {
    vi.stubGlobal('ResizeObserver', FakeObserver);
    const host = mount(() => (
      <VirtualList items={rows(200)} key={(r) => r.id} estimate={30} viewport={300} overscan={2}>
        {(r) => <span data-id={r().id}>{r().text}</span>}
      </VirtualList>
    ));
    const ro = FakeObserver.last;
    const box = host.querySelector<HTMLElement>('[data-kit="virtual-list"]');
    if (!ro || !box) throw new Error('no list');
    const shownIds = (): string[] => [...host.querySelectorAll('[data-id]')].map((n) => n.getAttribute('data-id') ?? '');
    const before = new Set(shownIds());
    // the rows are really 10px: the window takes in more rows at once
    ro.report((el) => (el === box ? { width: 390, height: 300 } : { width: 390, height: 10 }));
    const added = shownIds().filter((id) => !before.has(id));
    expect(added.length, 'the re-plan mounted rows in the same pass').toBeGreaterThan(0);
    const watchedIds = (): Set<string> => new Set([...ro.watched].map((el) => el.querySelector('[data-id]')?.getAttribute('data-id') ?? ''));
    expect(added.filter((id) => watchedIds().has(id)), 'not observed in that pass').toEqual([]);
    await new Promise<void>((r) => requestAnimationFrame(() => r()));
    expect(added.filter((id) => !watchedIds().has(id)), 'observed on the next frame').toEqual([]);
  });
});
