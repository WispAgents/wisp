// The way back to the Grid from a cloud wisp (docs/ux/2026-11-18-grid.md #9).
import { expect, test } from 'vitest';
import { gridHome, offerReach } from '../src/back.ts';
import type { CloudRow } from '../src/commands.ts';

const row = (home: string | undefined): CloudRow => ({ signedIn: false, grid: '', handle: '', machine: '', computer: null, signin: null, ...(home !== undefined && { home }) });

test('a_box_links_back_to_its_grid_s_home', () => {
  expect(gridHome(row('https://grid.trywisp.dev'))).toBe('https://grid.trywisp.dev/');
  expect(gridHome(row('http://wisp.localhost:8391'))).toBe('http://wisp.localhost:8391/');
});

test('anything_but_a_bare_grid_address_is_no_link', () => {
  for (const bad of ['', 'grid.trywisp.dev', 'javascript:alert(1)', 'https://g.example/x', 'https://u@g.example']) expect(gridHome(row(bad)), bad).toBeUndefined();
  expect(gridHome(row(undefined))).toBeUndefined();
  expect(gridHome(undefined)).toBeUndefined();
});

test('the_reach_tip_shows_only_off_a_grid_and_only_once_the_row_says_so', () => {
  expect(offerReach(undefined), 'not before the row lands').toBe(false);
  expect(offerReach(row(''))).toBe(true);
  expect(offerReach(row('https://grid.trywisp.dev')), 'a box is on a grid already').toBe(false);
});
