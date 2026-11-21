// @vitest-environment happy-dom
// Every plugin UI in this checkout registers in a real shell (apps/web/src/plugins.ts). main.tsx keeps
// going when one is refused (a bad plugin never stops the app), so a refusal, such as a string outside the
// plugin's namespace, used to drop a whole screen without a word: the open task mounted nothing
// (docs/v2-round-7.md). This test makes that loud, and checks the list has its plugin (the task screen is the frame's own until
// shell-task's is built).
import { expect, test } from 'vitest';
import { createClient, peerId } from '@wisp/client';
import { createShell, nullPlatform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
import { pluginUis } from '../src/plugins.ts';

test('every plugin UI registers, and the frame areas have their plugins', () => {
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform(), home, window: window as unknown as ShellWindow });
  const refused: string[] = [];
  for (const { path, def } of pluginUis()) {
    if (!def) continue;
    try {
      shell.register(def);
    } catch (err) {
      refused.push(`${path}: ${String(err)}`);
    }
  }
  expect(refused).toEqual([]);
  const areas = pluginUis().flatMap(({ def }) => (def?.extensions ?? []).map((e) => e.attachTo.id));
  for (const area of ['shell.sidebar:list', 'shell.task:band', 'shell.task:foot', 'shell.task:composer-top', 'shell.side:changes']) expect(areas, area).toContain(area);
});

// v1 composer.js `keyedRows(list, [...queuedItems(s), ...followUpItems(s)])`: one list inside the reply box over its
// text, the queued follow-ups first, then the ones set for a time, as the shell resolves the input's order
test('the_queued_follow_ups_sit_in_the_reply_box_before_the_scheduled_ones_v1_c_queued', () => {
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform(), home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def) shell.register(def);
  const ids = shell.context.extensions('shell.task:composer-top')().map((e) => e.id);
  expect(ids.indexOf('shell-composer.queue')).toBeGreaterThanOrEqual(0);
  expect(ids.indexOf('shell-composer.queue')).toBeLessThan(ids.indexOf('wisp.later.foot'));
  expect(shell.context.extensions('shell.task:foot')().map((e) => e.id)).not.toContain('shell-composer.queue');
});

// every later part, loaded while vitest collects this file, off the test's clock (the dynamic-import-in-test rule's
// reason: a dozen cold chunks inside one test's 5 s failed it under the full suite's load)
const withLater = await Promise.all(
  pluginUis().flatMap(({ path, def }) => (def?.later ? [{ path, def, later: def.later }] : [])).map(async (x) => ({ ...x, part: (await x.later()).default })),
);

test('every plugin UI that has a later part (ADR-0067) loads it, and it holds only what a press reaches, its ids its own', () => {
  expect(withLater.length).toBeGreaterThan(5);
  for (const { path, def, part } of withLater) {
    expect(Object.keys(part).filter((k) => !['extensions', 'palette', 'shortcuts', 'prompt', 'sideTabs'].includes(k)), path).toEqual([]);
    const ns = def.id.replace(/^wisp\./, '');
    for (const e of part.extensions ?? []) expect(e.id.startsWith(`${ns}.`) || e.id.startsWith(`${def.id}.`), `${path}: ${e.id}`).toBe(true);
  }
});
