// Mount a component in happy-dom and clean up after each test.
import { afterEach } from 'vitest';
import { render } from 'solid-js/web';
import type { JSX } from 'solid-js';

const mounted: Array<() => void> = [];

afterEach(() => {
  while (mounted.length) mounted.pop()?.();
  document.body.innerHTML = '';
});

export function mount(view: () => JSX.Element): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  mounted.push(render(view, host));
  return host;
}

export const tick = (ms = 0): Promise<void> => new Promise((r) => setTimeout(r, ms));
/** until `ok`, checked after each macrotask (what the test then asserts, never a fixed count of timer hops: on Windows
 * each is a ~15.6 ms system tick); gives up after `ms` by the clock, and the assertion after it fails with its message */
export async function until(ok: () => boolean, ms = 4000): Promise<void> {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await tick();
}

/** After the next animation frame (happy-dom runs them in order, as setImmediate): Zag starts an open overlay's focus
 * trap and its Escape and outside-press listeners in a frame, so a test that presses keys next awaits this, not only a
 * timer (a busy machine can fire a timer before a frame that was asked for earlier). */
export const frame = (): Promise<void> => new Promise((r) => requestAnimationFrame(() => r()));

// a data-part is a list of names (an element may answer to two, as v1's did): match one word
export const part = (name: string): HTMLElement | null => document.querySelector<HTMLElement>(`[data-part~="${name}"]`);
