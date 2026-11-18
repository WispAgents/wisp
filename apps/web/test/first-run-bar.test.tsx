// @vitest-environment happy-dom
// New task's bar never changes mode while it loads (ADR-0017, nothing jumps). The first-run bar (v1 launcher.js
// firstRun: the prompt, how it runs and Send) and the full one were decided before the task list was in: the bar drew
// full, went first-run when the saved "opened" answer came in with no task of yours yet, then full again when your
// tasks arrived. Each change replaced the bar's tools, so a tap on More in that window was lost. Now the bar keeps
// what it drew until the data says what it is: your tasks (full), or the list really empty (first run).
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId } from '@wisp/client';
import { createShell, nullPlatform, type Platform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
import { createMockPeer } from '../dev/mock-peer.ts';
import { memTransport } from '../dev/mem-transport.ts';
import { until } from './until.ts';

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  document.documentElement.removeAttribute('data-platform');
  document.body.innerHTML = '';
});

/** as main.tsx does: the client started, then the shell drawn, the mock's frames still coming (a cold start: nothing
 *  saved in this browser) */
async function start(platform: Platform, o: { firstRun?: boolean; width?: number } = {}) {
  const mock = createMockPeer({ epoch: 'e1', ...(o.firstRun && { firstRun: true }) });
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: o.width ?? 390 });
  const shell = createShell({ client, toaster: createToaster(), platform, home, window: window as unknown as ShellWindow });
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  return { client, root };
}

/** the bar's mode on each change from its first draw: full has Delegate, a first run keeps it behind More options */
function watchMode(root: HTMLElement): { modes: string[]; stop: () => void } {
  const modes: string[] = [];
  const note = (): void => {
    if (root.querySelector('[data-part~="launcher.input"]') === null) return;
    const mode = root.querySelector('[data-part~="launcher.team"]') === null ? 'first' : 'full';
    if (modes.at(-1) !== mode) modes.push(mode);
  };
  const mo = new MutationObserver(note);
  mo.observe(root, { childList: true, subtree: true, attributes: true });
  return { modes, stop: () => mo.disconnect() };
}

const phone: Platform = nullPlatform({ kind: 'android', phone: true, mobile: true });
const browser: Platform = nullPlatform({ kind: 'browser' });

for (const [where, platform] of [
  ['phone', phone],
  ['browser', browser],
] as const) {
  test(`on_a_${where}_with_tasks_new_tasks_bar_draws_full_and_never_flips_to_the_first_run_bar_while_it_loads`, async () => {
    const { client, root } = await start(platform, where === 'browser' ? { width: 1280 } : {});
    const w = watchMode(root);
    if (where === 'phone') {
      await until(() => root.querySelector('[data-part~="phone.tasks.new"]') !== null);
      root.querySelector<HTMLElement>('[data-part~="phone.tasks.new"]')?.click();
    }
    await until(() => client.entities.list('session')().length > 0 && w.modes.length > 0);
    // a few frames more: the saved answer and the list both in
    await until(() => false, 300);
    w.stop();
    expect(w.modes, 'one mode from its first draw').toEqual(['full']);
  });
}

test('with_no_task_yet_the_bar_settles_on_the_first_run_once_and_stays_there', async () => {
  const { client, root } = await start(browser, { firstRun: true, width: 1280 });
  const w = watchMode(root);
  await until(() => client.entities.loaded(peerId('local'), 'list')() && w.modes.at(-1) === 'first');
  await until(() => false, 300);
  w.stop();
  // drawn before the list is in (as full: a returning user's bar), then the first run once the list says so; never back
  expect(w.modes.at(-1), 'the first run').toBe('first');
  expect(w.modes.filter((m) => m === 'first'), 'settled once').toHaveLength(1);
  expect(w.modes.length, 'at most one change').toBeLessThanOrEqual(2);
});
