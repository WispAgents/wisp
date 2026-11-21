// Pull to refresh shows its spinner at once and holds it until the reconnect settles (ADR-0017), even when the
// status only flips to `connecting` on the next tick; a no-op reconnect still spins briefly and clears.
import { createEffect, createRoot, createSignal } from 'solid-js';
import { expect, test } from 'vitest';
import { createPull, type After } from '../src/model/pull.ts';

// Without this package's vitest.config.ts (e.g. `--config` pointing elsewhere, or a root run before the root
// vite.config.ts made each package a project) `solid-js` resolves to its
// server build, where createEffect is a no-op: the pull never sees the status flip and the first test fails with a
// bare 'expected false to be true' (the others pass for the wrong reason). Say so instead.
function effectsRun(): boolean {
  let ran = false;
  const dispose = createRoot((d) => {
    createEffect(() => {
      ran = true;
    });
    return d;
  });
  dispose();
  return ran;
}

function harness() {
  expect(effectsRun(), 'solid-js resolved to its server build (createEffect is a no-op): run it under plugins/shell-list/ui/vitest.config.ts (from that folder, or from the root via vite.config.ts)').toBe(true);
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
