// B30: a feed row learns its time's width (--k-time-w) so the quick actions sit before a wide time, never on it.
import { expect, test, vi } from 'vitest';

const ro = vi.hoisted(() => {
  const s: { cb?: (entries: { target: Element; borderBoxSize: { inlineSize: number }[]; contentRect: { width: number } }[]) => void; seen: Set<Element> } = { seen: new Set() };
  class Fake {
    constructor(cb: NonNullable<typeof s.cb>) {
      s.cb = cb;
    }
    observe(el: Element): void {
      s.seen.add(el);
    }
    unobserve(el: Element): void {
      s.seen.delete(el);
    }
  }
  (globalThis as { ResizeObserver?: unknown }).ResizeObserver = Fake;
  return s;
});

import { watchTimeWidth } from '../src/composites/time-width.ts';

test('the row gets its time width, and loses it (and the watch) when stopped', () => {
  const row = document.createElement('div');
  const time = document.createElement('time');
  const stop = watchTimeWidth(row, time);
  expect(ro.seen.has(time)).toBe(true);
  ro.cb?.([{ target: time, borderBoxSize: [{ inlineSize: 31.4 }], contentRect: { width: 31.4 } }]);
  expect(row.style.getPropertyValue('--k-time-w')).toBe('32px');
  stop();
  expect(ro.seen.has(time)).toBe(false);
  expect(row.style.getPropertyValue('--k-time-w')).toBe('');
});

test('the same width reported again writes nothing: rows aren’t restyled by changes that aren’t theirs_time_width_dedupe', () => {
  const row = document.createElement('div');
  const time = document.createElement('time');
  const set = vi.spyOn(row.style, 'setProperty');
  const stop = watchTimeWidth(row, time);
  const report = (w: number): void => ro.cb?.([{ target: time, borderBoxSize: [{ inlineSize: w }], contentRect: { width: w } }]);
  report(31.4);
  report(31.6);
  report(32);
  expect(set).toHaveBeenCalledTimes(1);
  report(40);
  expect(set).toHaveBeenCalledTimes(2);
  expect(row.style.getPropertyValue('--k-time-w')).toBe('40px');
  stop();
  // watched again (a re-render): the width is written again, once
  const again = watchTimeWidth(row, time);
  report(40);
  report(40);
  expect(set).toHaveBeenCalledTimes(3);
  again();
});
