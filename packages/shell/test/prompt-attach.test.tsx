// Files shared to the phone app go to New task's attachments (mobile/web share-in.ts): New task mounts after the share
// navigates there, so files for a box not shown yet wait for it and are handed over the moment it shows, once.
import { expect, test } from 'vitest';
import { createClient, peerId } from '@wisp/client';
import { createToaster } from '@wisp/ui';
import { createShell, type ShellWindow } from '../src/shell.tsx';
import { nullPlatform } from '../src/platform/index.ts';

test('files_for_a_prompt_box_wait_until_it_shows_then_go_to_it_once', () => {
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 390 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform(), home, window: window as unknown as ShellWindow });
  const prompts = shell.context.prompts;
  const a = new File(['a'], 'a.png', { type: 'image/png' });
  const b = new File(['b'], 'b.pdf', { type: 'application/pdf' });
  prompts?.attach?.('launcher', [a]);
  prompts?.attach?.('launcher', [b]);
  const got: File[] = [];
  const box = { text: () => '', set: () => {}, focus: () => {}, attach: (fs: readonly File[]) => void got.push(...fs) };
  const off = prompts?.provide('launcher', box);
  expect(got).toEqual([a, b]);
  // shown again (Home mounted anew): nothing twice; while shown, straight to it
  off?.();
  prompts?.provide('launcher', box);
  expect(got).toEqual([a, b]);
  const c = new File(['c'], 'c.txt');
  prompts?.attach?.('launcher', [c]);
  expect(got).toEqual([a, b, c]);
  shell.dispose();
  client.dispose();
});
