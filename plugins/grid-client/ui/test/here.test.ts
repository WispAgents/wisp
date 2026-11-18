import { expect, test } from 'vitest';
import type { Computer } from '../src/commands.ts';
import { hereOf } from '../src/here.ts';
import { t } from '../src/i18n.ts';

const k = (x: Partial<Computer>): Computer => ({ on: true, state: 'online', label: '', url: '', error: '', ...x });

// TRY-17 #14: Settings → Machines' This computer said "Online" and never where; its address on the grid is a link
test('this_computer_on_the_grid_shows_its_address_as_a_link_once_it_is_on', () => {
  expect(hereOf(k({ url: 'https://studio--ada.grid.trywisp.dev' }), t)).toEqual({ line: t('grid-client.computer.onlineNoUrl'), url: 'https://studio--ada.grid.trywisp.dev' });
  // connecting or trying again: where it will be reached, with its state
  expect(hereOf(k({ state: 'retrying', error: 'grid down', url: 'https://a.g' }), t)).toEqual({ line: 'grid down', url: 'https://a.g' });
  expect(hereOf(k({ state: 'connecting' }), t)).toEqual({ line: t('grid-client.computer.connecting') });
  // off, or no account's computer: no address
  expect(hereOf(k({ on: false, url: 'https://a.g' }), t)).toEqual({ line: t('grid-client.computer.off') });
  expect(hereOf(null, t)).toEqual({ line: t('grid-client.computer.off') });
  // only a web address is a link
  expect(hereOf(k({ url: 'javascript:alert(1)' }), t).url).toBeUndefined();
});

// QA round 9 #1: removed on the Grid's Home, this computer is off here, says so, and Turn on (Add to grid) adds it anew
test('a_computer_removed_on_the_grid_is_off_and_says_turning_it_on_adds_it_again', () => {
  expect(hereOf(k({ on: false, state: 'gone', error: 'The grid doesn’t know this computer any more.' }), t)).toEqual({
    line: t('grid-client.computer.removed'),
  });
  expect(t('grid-client.computer.removed')).not.toBe('grid-client.computer.removed');
  // still on but refused (this device's sign-in ended): why, never "Connecting…"
  expect(hereOf(k({ state: 'gone', error: 'Sign in again.' }), t)).toEqual({ line: 'Sign in again.' });
});
