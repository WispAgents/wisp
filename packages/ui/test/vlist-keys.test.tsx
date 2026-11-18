// Perf audit 2026-11-18 #14: a measurement pass (each scroll frame that brings rows in) mapped all 5,000 items through
// `key` twice, once for the offsets and once to find the anchor row. Keys and their indexes are now built once per
// change of items; a pass reads heights by key and finds the anchor in a map.
import { afterEach, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { mount } from './helpers.tsx';
import { VirtualList } from '../src/layout/VirtualList.tsx';

class FakeObserver {
  static last: FakeObserver | undefined;
  readonly watched = new Set<Element>();
  private readonly cb: (entries: ResizeObserverEntry[]) => void;
  constructor(cb: (entries: ResizeObserverEntry[]) => void) {
    this.cb = cb;
    FakeObserver.last = this;
  }
  observe(el: Element): void {
    this.watched.add(el);
  }
  unobserve(el: Element): void {
    this.watched.delete(el);
  }
  disconnect(): void {
    this.watched.clear();
  }
  report(size: (el: Element) => number): void {
    this.cb([...this.watched].map((target) => ({ target, borderBoxSize: [{ inlineSize: 390, blockSize: size(target) }], contentRect: { width: 390, height: size(target) } }) as unknown as ResizeObserverEntry));
  }
}

afterEach(() => vi.unstubAllGlobals());

const rows = (n: number, from = 0) => Array.from({ length: n }, (_, i) => ({ id: `r${i + from}` }));

test('a measurement pass calls key for no row at all, whatever the list’s length_vlist_keys_once', () => {
  vi.stubGlobal('ResizeObserver', FakeObserver);
  let calls = 0;
  const key = (r: { id: string }): string => {
    calls++;
    return r.id;
  };
  const host = mount(() => (
    <VirtualList items={rows(5000)} key={key} estimate={30} viewport={300} overscan={2}>
      {(r) => <span data-id={r().id} />}
    </VirtualList>
  ));
  const ro = FakeObserver.last;
  if (!ro) throw new Error('no observer');
  const box = host.querySelector('[data-kit="virtual-list"]');
  const before = calls;
  // three passes with new heights (each re-plans the window)
  for (const h of [31, 32, 33]) ro.report((el) => (el === box ? 300 : h));
  // only the rendered rows' own keys (Key's identity for the few rows that move in or out), never the 5,000
  expect(calls - before).toBeLessThan(200);
});

test('rows inserted above the one being read: it stays where it was (anchor found by key)_vlist_keys_anchor', () => {
  vi.stubGlobal('ResizeObserver', FakeObserver);
  const [items, setItems] = createSignal(rows(1000, 100));
  const host = mount(() => (
    <VirtualList items={items()} key={(r) => r.id} estimate={30} viewport={300} overscan={2}>
      {(r) => <span data-id={r().id} />}
    </VirtualList>
  ));
  const list = host.querySelector<HTMLElement>('[data-kit="virtual-list"]');
  if (!list) throw new Error('no list');
  list.scrollTop = 30 * 500;
  list.dispatchEvent(new Event('scroll'));
  // 100 rows arrive above: the reader's row (r600) is 3,000 px further down, so the scroll follows it
  setItems([...rows(100, 0), ...items()]);
  expect(list.scrollTop).toBe(30 * 600);
});
