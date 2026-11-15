// Pull to refresh shows its spinner at once and holds it until the reconnect settles (ADR-0017), even when the
// status only flips to `connecting` on the next tick; a no-op reconnect still spins briefly and clears.
import { createRoot, createSignal } from 'solid-js';
import { expect, test } from 'vitest';
import { createPull, type After } from '../src/model/pull.ts';

function harness() {
  const [connecting, setConnecting] = createSignal(false);
  const timers: Array<() => void> = [];
  const after: After = (_ms, run) => {
    timers.push(run);
    return () => timers.splice(timers.indexOf(run), 1);
  };
  let reconnects = 0;
  const pull = createRoot(() => createPull(connecting, () => reconnects++, after, 600));
  return { pull, setConnecting, fire: () => timers.shift()?.(), reconnects: () => reconnects };
}

test('pull spinner shows at once when the status flips to connecting on the next tick, and clears once connected', async () => {
  const h = harness();
  h.pull.pull();
  expect(h.reconnects()).toBe(1);
  expect(h.pull.refreshing()).toBe(true); // same frame, before the status moves
  await Promise.resolve();
  h.setConnecting(true); // next tick
  h.fire(); // the brief spin is over, still connecting
  expect(h.pull.refreshing()).toBe(true);
  h.setConnecting(false); // online again (or failed: the offline bar shows why)
  expect(h.pull.refreshing()).toBe(false);
});

test('pull spinner holds for the brief spin even if the reconnect settles faster', () => {
  const h = harness();
  h.pull.pull();
  h.setConnecting(true);
  h.setConnecting(false);
  expect(h.pull.refreshing()).toBe(true);
  h.fire();
  expect(h.pull.refreshing()).toBe(false);
});

test('pull on a healthy connection (reconnect is a no-op) spins briefly and clears', () => {
  const h = harness();
  h.pull.pull();
  expect(h.pull.refreshing()).toBe(true);
  h.fire();
  expect(h.pull.refreshing()).toBe(false);
});
