// @vitest-environment happy-dom
// doc924a review: Settings' search sends budget.autoStopCodex to Agents on Codex with that key as its focus; the stop
// line's row had no key, so nothing was lit or focused. It carries its declared key now.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { signal } from '@wisp/client';
import { KitProvider } from '@wisp/ui';
import HarnessStop from '../src/HarnessStop.tsx';

afterEach(() => {
  document.body.innerHTML = '';
});

test('a_harness_stop_line_row_carries_its_declared_key', () => {
  const scope = { child: () => scope, dispose: () => undefined, interval: () => () => undefined, timers: { now: () => 0 } };
  const client = {
    home: 'laptop',
    scope,
    watch: () => undefined,
    entities: { row: () => signal(undefined), list: () => signal([]) },
    run: () => new Promise(() => undefined),
  };
  const host = document.createElement('div');
  document.body.append(host);
  const stop = render(
    () => (
      <KitProvider t={(k) => k}>
        <HarnessStop client={client as never} config={undefined as never} id="codex" />
      </KitProvider>
    ),
    host,
  );
  expect(host.querySelector('[data-setting-key="budget.autoStopCodex"]')).not.toBeNull();
  stop();
});
