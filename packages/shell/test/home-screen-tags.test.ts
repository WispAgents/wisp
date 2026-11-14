// F-phone-6 on a Grid only (v1 homescreen.js): the manifest and Apple's Home Screen tags go into a grid box's page,
// never a browser on someone's own computer, never the native apps.
import { expect, test } from 'vitest';
import { addHomeScreenTags, HOME_SCREEN_TAGS, homeScreenTags, nativeShell } from '../src/platform/standalone.ts';

test('in_a_grid_box_the_page_gets_the_home_screen_tags_and_locally_none', () => {
  expect(homeScreenTags({ inBox: true, native: false })).toBe(HOME_SCREEN_TAGS);
  expect(homeScreenTags({ inBox: false, native: false }), 'own computer').toEqual([]);
  expect(homeScreenTags({ inBox: true, native: true }), 'the native apps').toEqual([]);
  expect(homeScreenTags({ inBox: false, native: true })).toEqual([]);
  const names = HOME_SCREEN_TAGS.map((t) => t.attrs['rel'] ?? t.attrs['name']);
  expect(names).toEqual(['manifest', 'apple-mobile-web-app-capable', 'mobile-web-app-capable', 'apple-mobile-web-app-status-bar-style', 'apple-mobile-web-app-title', 'theme-color', 'theme-color']);
  expect(HOME_SCREEN_TAGS.find((t) => t.attrs['name'] === 'apple-mobile-web-app-status-bar-style')?.attrs['content']).toBe('black-translucent');
  expect(HOME_SCREEN_TAGS.filter((t) => t.attrs['name'] === 'theme-color').map((t) => [t.attrs['media'], t.attrs['content']])).toEqual([
    ['(prefers-color-scheme: light)', '#ffffff'],
    ['(prefers-color-scheme: dark)', '#111111'],
  ]);
});

test('the_native_apps_are_told_apart_from_a_browser', () => {
  const ua = (userAgent: string) => ({ navigator: { userAgent } });
  expect(nativeShell(ua('Mozilla/5.0 (iPhone) Safari'))).toBe(false);
  expect(nativeShell({ ...ua('x'), Capacitor: { isNativePlatform: () => false } }), 'Capacitor core in a browser').toBe(false);
  expect(nativeShell({ ...ua('x'), Capacitor: { isNativePlatform: () => true } })).toBe(true);
  expect(nativeShell({ ...ua('x'), wispAndroid: {} })).toBe(true);
  expect(nativeShell(ua('Mozilla/5.0 wispMobile/1'))).toBe(true);
  expect(nativeShell({ ...ua('x'), __TAURI_INTERNALS__: {} }), 'the desktop app').toBe(true);
});

test('the_tags_are_added_to_the_head_once', () => {
  const head: { tag: string; attrs: Record<string, string> }[] = [];
  const doc = {
    head: { append: (n: { tag: string; attrs: Record<string, string> }) => head.push(n) },
    createElement: (tag: string) => ({ tag, attrs: {} as Record<string, string>, setAttribute(k: string, v: string) { this.attrs[k] = v; } }),
    querySelector: (q: string) => (q === 'link[rel="manifest"]' && head.some((n) => n.attrs['rel'] === 'manifest') ? {} : null),
  } as unknown as Parameters<typeof addHomeScreenTags>[0];
  expect(addHomeScreenTags(doc, [])).toBe(false);
  expect(head).toEqual([]);
  expect(addHomeScreenTags(doc, HOME_SCREEN_TAGS)).toBe(true);
  expect(head.map((n) => ({ tag: n.tag, attrs: n.attrs }))).toEqual(HOME_SCREEN_TAGS.map((t) => ({ tag: t.tag, attrs: t.attrs })));
  expect(addHomeScreenTags(doc, HOME_SCREEN_TAGS), 'a second call').toBe(false);
  expect(head).toHaveLength(HOME_SCREEN_TAGS.length);
});
