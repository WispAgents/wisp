// QA round 3 desktop #9: a reload (or the desktop app's relaunch, which restores the last place) brought back an open
// ⌘K palette: the URL kept `?o=dialog:shell.palette`. The last place comes back, never the search box over it (v1).
import { expect, test } from 'vitest';
import { createClient, peerId } from '@wisp/client';
import { createToaster } from '@wisp/ui';
import { createShell, type ShellWindow } from '../src/shell.tsx';
import { nullPlatform } from '../src/platform/index.ts';

test('a_restored_palette_is_closed_as_the_app_draws_and_the_place_under_it_stays', () => {
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 1440 });
  const shell = createShell({ client, toaster: createToaster(), platform: nullPlatform(), home, window: window as unknown as ShellWindow });
  // what a reload restored: a view, with the palette over it
  client.nav.navigate({ t: 'openView', id: 'crew.org' as never, params: null });
  client.nav.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.palette' as never, params: null } });
  expect(client.nav.nav().overlays.map((o) => o.id)).toEqual(['shell.palette']);
  const under = client.nav.nav().main;
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  expect(client.nav.nav().overlays).toEqual([]);
  expect(client.nav.nav().main, 'the place under it stays').toEqual(under);
  stop();
  shell.dispose();
});
