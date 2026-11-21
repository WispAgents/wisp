// The flow block grew without end in the task view: its wires svg sat in the stage's flow with a viewBox
// of the stage's size, so every measure made the stage taller and the ResizeObserver measured again.
import css from '../src/css/answer-rich.css?raw';
import { afterEach, expect, test, vi } from 'vitest';
import { mount } from './helpers.tsx';
import { Answer, type BlockContext } from '../src/index.ts';

const ro = vi.hoisted(() => {
  const s: { cb?: (entries: { contentRect: { width: number } }[]) => void } = {};
  class Fake {
    constructor(cb: NonNullable<typeof s.cb>) {
      s.cb = cb;
    }
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
  }
  (globalThis as { ResizeObserver?: unknown }).ResizeObserver = Fake;
  return s;
});

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 20));

afterEach(() => vi.restoreAllMocks());

test('a stage that keeps growing at one width stops being measured; a new width measures again_flow_no_runaway', async () => {
  vi.stubGlobal('requestAnimationFrame', (run: FrameRequestCallback) => setTimeout(() => run(0), 0));
  vi.stubGlobal('cancelAnimationFrame', (id: number) => clearTimeout(id));
  const ctx: BlockContext = { latest: true };
  mount(() => (
    <Answer
      ctx={ctx}
      blocks={[{ type: 'flow', title: 'Why', nodes: [{ id: 'a', label: 'A' }, { id: 'b', label: 'B' }, { id: 'c', label: 'C' }] }]}
    />
  ));
  const stage = document.querySelector('[data-kit="rb-flow-stage"]');
  expect(stage).not.toBeNull();
  let width = 800;
  let height = 100;
  let reads = 0;
  vi.spyOn(stage as Element, 'getBoundingClientRect').mockImplementation(() => {
    reads++;
    height += 100; // the feedback: each measure makes the stage taller
    return { left: 0, top: 0, right: width, bottom: height, width, height, x: 0, y: 0, toJSON: () => ({}) } as DOMRect;
  });
  for (let i = 0; i < 30; i++) {
    ro.cb?.([{ contentRect: { width } }]);
    await flush();
  }
  const capped = reads;
  expect(capped).toBeGreaterThan(0);
  expect(capped).toBeLessThanOrEqual(10);
  width = 600;
  ro.cb?.([{ contentRect: { width } }]);
  await flush();
  expect(reads).toBeGreaterThan(capped);
});

test('the wires svg is laid over the stage, never in its flow_flow_wires_absolute', () => {
  const rule = /\[data-kit='rb-flow-wires'\]\s*\{([^}]*)\}/.exec(css)?.[1] ?? '';
  expect(rule).toMatch(/position:\s*absolute/);
  expect(rule).toMatch(/inset:\s*0/);
  expect(/\[data-kit='rb-flow-stage'\]\s*\{[^}]*position:\s*relative/.test(css)).toBe(true);
});

test('a still answer draws its wires at once: no animation leaves them hidden_flow_still', () => {
  const rule = /\[data-still\] \[data-kit='rb-flow-wire'\]\s*\{([^}]*)\}/.exec(css)?.[1] ?? '';
  expect(rule).toMatch(/animation:\s*none/);
  expect(rule).toMatch(/stroke-dashoffset:\s*0/);
});
