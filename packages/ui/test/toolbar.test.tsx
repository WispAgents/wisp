// The no-wrapping-bars rule (design/layout.md §2): items step down lowest priority first, label to
// icon before anything moves into More, pins never step down, and expanding waits for slack.
import { afterEach, beforeAll, beforeEach, describe, expect, test, vi } from 'vitest';
import { mount, part, tick, until } from './helpers.tsx';
import { Toolbar, collapseSteps, formsAt, levelFor, nextLevel, widthAt, type PlanInput, type ToolbarItem } from '../src/index.ts';
import { preloadControls } from '../src/controls/lazy-part.ts';
import { inObserverPass } from '../src/core/observer-scope.ts';

// the app fetches the overlays' machines once its first screen is up (ADR-0011); a press before that: lazy-controls.test.tsx
beforeAll(() => preloadControls());

const item = (id: string, priority: 'pin' | number, full = 100, icon = 32) => ({ id, priority, full, icon });

describe('planToolbar', () => {
  const items = [item('mode', 3), item('queue', 2), item('attach', 1), item('send', 'pin', 80, 32)];
  const input = (available: number): PlanInput => ({ items, available, gap: 4, more: 32 });

  test('collapse order: every label to its icon (lowest priority first), then into More; pins never', () => {
    expect(collapseSteps(items)).toEqual([
      { id: 'attach', to: 'icon' }, { id: 'queue', to: 'icon' }, { id: 'mode', to: 'icon' },
      { id: 'attach', to: 'overflow' }, { id: 'queue', to: 'overflow' }, { id: 'mode', to: 'overflow' },
    ]);
  });

  test('on a priority tie the later item steps down first', () => {
    expect(collapseSteps([item('a', 1), item('b', 1)])[0]).toEqual({ id: 'b', to: 'icon' });
  });

  test('a wide bar keeps every label', () => {
    expect(levelFor(input(1000))).toBe(0);
  });

  test('the fewest steps that fit, and the result fits', () => {
    for (let available = 100; available <= 500; available += 7) {
      const level = levelFor(input(available));
      const w = widthAt(input(available), formsAt(items, level));
      if (level < collapseSteps(items).length) expect(w).toBeLessThanOrEqual(available);
      if (level > 0) expect(widthAt(input(available), formsAt(items, level - 1))).toBeGreaterThan(available);
    }
  });

  test('labels fold to icons before anything moves into More', () => {
    // full row: 100*3 + 80 + 3 gaps = 392. All icons: 32*3 + 80 + 12 = 188.
    expect(formsAt(items, levelFor(input(300))).get('attach')).toBe('icon');
    const tight = formsAt(items, levelFor(input(190)));
    expect([...tight.values()].filter((f) => f === 'overflow')).toEqual([]);
    expect(formsAt(items, levelFor(input(150))).get('attach')).toBe('overflow');
  });

  test('a pinned item is never collapsed, even when nothing fits', () => {
    expect(formsAt(items, levelFor(input(10))).get('send')).toBe('full');
  });

  test('hysteresis: collapsing is immediate, expanding needs one item of slack', () => {
    const at = levelFor(input(300));
    const just = widthAt(input(300), formsAt(items, at - 1));
    // Exactly enough room to expand one step: without slack it stays collapsed.
    expect(nextLevel(input(just), at, 32)).toBe(at);
    expect(nextLevel(input(just + 32), at, 32)).toBe(at - 1);
    expect(nextLevel(input(120), 0, 32)).toBe(levelFor(input(120)));
  });
});

describe('Toolbar collapses as measured', () => {
  type Cb = (entries: ResizeObserverEntry[]) => void;
  const observers: Array<{ cb: Cb; els: Element[] }> = [];
  const watching = (key: string): boolean => observers.some((o) => o.els.some((el) => (el as HTMLElement).dataset['measure'] === key));
  beforeEach(() => {
    class FakeRO {
      private readonly rec: { cb: Cb; els: Element[] };
      constructor(cb: Cb) {
        this.rec = { cb, els: [] };
        observers.push(this.rec);
      }
      observe(el: Element): void {
        if (!this.rec.els.includes(el)) this.rec.els.push(el);
      }
      disconnect(): void {}
      unobserve(el: Element): void {
        this.rec.els = this.rec.els.filter((x) => x !== el);
      }
    }
    vi.stubGlobal('ResizeObserver', FakeRO);
  });
  afterEach(async () => {
    vi.unstubAllGlobals();
    observers.length = 0;
    // `resize` delivers observer callbacks, which open a kit observer pass (core/observer-scope.ts) that ends at the next
    // task. A test that starts inside it observes its new toolbar a frame late (observeSoon), so its first `resize`
    // reaches no one: whether `tick()` or that frame came first was the Windows flake (main 44878585321, all `full`).
    await until(() => !inObserverPass());
    expect(inObserverPass(), 'the next test starts outside an observer pass').toBe(false);
  });

  /** Report widths for every observed element: the bar's width, 100 per label form, 32 per icon. */
  // the bar's laid-out width (`offsetWidth`, its border box, which the toolbar compares with what its observer reported):
  // what the last `resize` reported, unless a test says how a fold changes it
  let laid = 0;
  const resize = (bar: number): void => {
    laid = bar;
    for (const o of observers) {
      for (const el of o.els) {
        if ((el as HTMLElement).dataset['measure'] === 'bar' && !Object.getOwnPropertyDescriptor(el, 'offsetWidth')) {
          Object.defineProperty(el, 'offsetWidth', { configurable: true, get: () => laid });
        }
      }
      const entries = o.els.map((el) => {
        const key = (el as HTMLElement).dataset['measure'] ?? '';
        const width = key === 'bar' ? bar : key.endsWith(':full') ? 100 : 32;
        return { target: el, contentRect: { width }, borderBoxSize: [{ inlineSize: width, blockSize: 32 }] } as unknown as ResizeObserverEntry;
      });
      o.cb(entries);
    }
  };

  const noop = (): void => {};
  const items: ToolbarItem[] = [
    { id: 'mode', label: 'Full auto', icon: 'bolt', priority: 2, onSelect: noop },
    { id: 'attach', label: 'Attach', icon: 'clip', priority: 1, onSelect: noop },
    { id: 'send', label: 'Send', icon: 'send', priority: 'pin', onSelect: noop },
  ];
  const form = (id: string): string | null | undefined => document.querySelector(`[data-kit="toolbar-item"][data-item="${id}"]`)?.getAttribute('data-form');

  // QA round 2 #15: in an app bar the toolbar is as wide as its items, so folding one made the bar it measures
  // narrower in the same pass, which the browser skips: Chrome's "ResizeObserver loop completed with undelivered
  // notifications" on every load of Tasks. The fold lands at once; the bar steps out of the observer until the next
  // frame, which measures its new width.
  test('a_fold_that_resizes_the_bar_measures_it_again_next_frame_not_in_the_same_pass', async () => {
    mount(() => <Toolbar aria-label="Reply bar" items={items} part="reply.bar" />);
    await tick();
    const bar = document.querySelector<HTMLElement>('[data-kit="toolbar"]');
    if (!bar) throw new Error('no bar');
    // folding the mode button makes the bar (as wide as its items) 6 px narrower than the width just reported
    Object.defineProperty(bar, 'offsetWidth', { configurable: true, get: () => laid - (form('mode') === 'full' ? 0 : 6) });
    resize(400);
    expect(form('mode')).toBe('full');
    expect(watching('bar'), 'no width change, still watched').toBe(true);
    resize(200);
    expect(form('mode'), 'the fold lands in the same pass').toBe('icon');
    expect(watching('bar'), 'the bar it resized steps out').toBe(false);
    await new Promise<void>((r) => requestAnimationFrame(() => r()));
    expect(watching('bar'), 'measured again next frame').toBe(true);
  });

  test('wide: labels; narrower: icons; narrowest: More; the pin stays whole and nodes stay put', async () => {
    mount(() => <Toolbar aria-label="Reply bar" items={items} part="reply.bar" />);
    await tick();
    const sendNode = document.querySelector('[data-kit="toolbar-item"][data-item="send"]');
    resize(400);
    expect([form('mode'), form('attach'), form('send')]).toEqual(['full', 'full', 'full']);
    resize(200);
    expect([form('mode'), form('attach'), form('send')]).toEqual(['icon', 'icon', 'full']);
    expect(document.querySelector('[data-item="attach"] button')?.getAttribute('aria-label')).toBe('Attach');
    resize(150);
    expect(form('attach')).toBe('overflow');
    expect(document.querySelector('[data-item="attach"]')?.hasAttribute('hidden')).toBe(true);
    expect(part('reply.bar.more.trigger')).not.toBeNull();
    expect(form('send')).toBe('full');
    expect(document.querySelector('[data-kit="toolbar-item"][data-item="send"]')).toBe(sendNode);
  });

  test('labels are control text: one line', async () => {
    mount(() => <Toolbar aria-label="Reply bar" items={items} />);
    await tick();
    for (const el of document.querySelectorAll('[data-kit="toolbar-item"] [data-kit="button-label"]')) {
      expect(el.getAttribute('data-text')).toBe('control');
    }
  });

  test('More comes and goes without leaving menus behind', async () => {
    mount(() => <Toolbar aria-label="Reply bar" items={items} part="reply.bar" />);
    await tick();
    for (const w of [150, 400, 150, 400, 150]) resize(w);
    await tick();
    expect(document.querySelectorAll('[data-part="reply.bar.more.trigger"]').length).toBe(1);
    expect(document.querySelectorAll('[data-part="reply.bar.more"]').length).toBe(1);
  });
});
