// F-phone-6 on a Grid only: the page gets the manifest and Apple's tags once this wisp's row says it's a grid box,
// and none on someone's own computer (v1 homescreen.js).
import { afterEach, expect, test } from 'vitest';
import { createScope, signal, type PluginClient } from '@wisp/client';
import { watchHomeScreen } from '../src/homescreen.ts';

const fake = (row: () => unknown): { c: PluginClient; watched: string[] } => {
  const watched: string[] = [];
  const c = { home: 'p', scope: createScope('t'), watch: (_p: string, topic: string) => void watched.push(topic), entities: { row: () => row } };
  return { c: c as unknown as PluginClient, watched };
};
const manifests = (): number => document.head.querySelectorAll('link[rel="manifest"]').length;
const capable = (): string | null | undefined => document.head.querySelector('meta[name="apple-mobile-web-app-capable"]')?.getAttribute('content');

afterEach(() => {
  document.head.innerHTML = '';
});

test('a_grid_box_s_page_gets_the_home_screen_tags_once_its_row_lands', () => {
  const row = signal<unknown>(undefined);
  const { c, watched } = fake(row);
  watchHomeScreen(c);
  expect(watched).toEqual(['plugin:wisp.grid-client:cloud']);
  expect(manifests(), 'nothing before the row says where it runs').toBe(0);
  row({ preview: 'https://p{port}--ada.grid.example' });
  expect(manifests()).toBe(1);
  expect(capable()).toBe('yes');
  expect(document.head.querySelectorAll('meta[name="theme-color"]')).toHaveLength(2);
  row({ preview: 'https://p{port}--ada.grid.example', signedIn: true });
  expect(manifests(), 'added once').toBe(1);
});

test('a_wisp_on_someone_s_own_computer_never_offers_to_install', () => {
  const row = signal<unknown>({ signedIn: true, preview: '' });
  watchHomeScreen(fake(row).c);
  row({ signedIn: false });
  // its row is decided by the box's env: a later one changes nothing
  row({ preview: 'https://p{port}--ada.grid.example' });
  expect(manifests()).toBe(0);
  expect(capable()).toBeUndefined();
});
