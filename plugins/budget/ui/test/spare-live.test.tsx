// @vitest-environment happy-dom
// round 324: Home's Spare allowance card follows Settings → Usage's switch (budget.spareCard, a `setting` row in the
// entity store) while Home shows: turned off, the card goes at once; on again, it comes back. The memo read the row
// directly, which Solid doesn't follow, so the card stayed until Home was opened again.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { signal } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import Spare from '../src/Spare.tsx';
import { until } from './until.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

const H = 3_600_000;
const NOW = 1_000 * H;
const WEEK = 7 * 24 * H;

test('the_spare_card_goes_when_its_setting_is_turned_off_while_home_shows_and_comes_back_when_on', async () => {
  const setting = signal<{ key: string; value: unknown } | undefined>(undefined);
  const none = signal(undefined);
  const scope = { child: () => scope, dispose: () => undefined, interval: () => () => undefined, timers: { now: () => NOW } };
  const client = {
    home: 'laptop',
    scope,
    watch: () => undefined,
    entities: { row: (_kind: string, key: string) => (key === 'laptop:budget.spareCard' ? setting : none) },
    defineStore: () => ({ value: signal({}), set: () => undefined }),
    run: async (_cmd: unknown, a: { agent: string }) => (a.agent === 'codex' ? [{ account: 'default', at: NOW - H, windows: [{ id: 'weekly', label: 'Weekly', used: 0.62, resetsAt: NOW + 10 * H, ms: WEEK }] }] : []),
  };
  const shell = { harnesses: () => signal([]) };
  const host = document.createElement('div');
  document.body.append(host);
  const stop = render(
    () => (
      <KitProvider t={(k) => k}>
        <ShellProvider value={shell as never}>
          <Spare client={client as never} config={undefined as never} />
        </ShellProvider>
      </KitProvider>
    ),
    host,
  );
  const card = (): Element | null => host.querySelector('[data-part="home.spare"]');
  await until(() => card() !== null);
  expect(card(), 'unset is on: the card shows').not.toBeNull();
  setting({ key: 'budget.spareCard', value: 'off' });
  expect(card(), 'turned off: it goes at once').toBeNull();
  setting({ key: 'budget.spareCard', value: 'on' });
  expect(card(), 'on again: it comes back').not.toBeNull();
  stop();
});
