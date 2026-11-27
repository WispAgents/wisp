/// <reference types="vite/client" />
// The answer blocks' copy (`block.*`, the kit's BLOCK_STRINGS) is registered by the screens that show blocks (the task
// page, Compare) as they load, and fetched after the first page for the rest, not carried in the first page's bundle
// (budget, ADR-0011: initial JS ≤ 120 KB br). Registering is once: a second screen finds it there.
import { expect, test } from 'vitest';
import { createClient, peerId } from '@wisp/client';
import { BLOCK_STRINGS } from '@wisp/ui';
import shellSrc from '../src/shell.tsx?raw';
// the task page's state (task-page.ts, used by TaskView.tsx)
import taskView from '../src/frame/task-page.ts?raw';
import compare from '../src/frame/Compare.tsx?raw';
import { blockStrings } from '../src/block-strings.ts';

test('the_block_strings_load_with_the_screens_that_show_blocks_once_not_with_the_first_page', () => {
  // the shell no longer imports them at its start; it fetches them after
  expect(shellSrc).not.toMatch(/BLOCK_STRINGS/);
  expect(shellSrc).toMatch(/import\('\.\/block-strings\.ts'\)/);
  for (const src of [taskView, compare]) expect(src).toMatch(/blockStrings\(shell\.client\)/);
  const client = createClient({ clientId: 'c1', build: 't', home: peerId('local'), width: 1280 });
  const key = Object.keys(BLOCK_STRINGS)[0] as keyof typeof BLOCK_STRINGS;
  expect(client.i18n.has(key)).toBe(false);
  blockStrings(client);
  expect(client.i18n.t(key)).toBe(BLOCK_STRINGS[key]);
  // the next screen finds them there: no second registration (it would throw)
  expect(() => blockStrings(client)).not.toThrow();
});
