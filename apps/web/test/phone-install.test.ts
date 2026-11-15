// F-phone-6: given iPhone Safari on my cloud wisp, when I Add to Home Screen, then it runs full screen. The page
// (index.html as built: dist/, which `wisp serve` serves) leaves what iOS and Chrome read to install it to a grid
// box (added at runtime), and the head script that marks a Home Screen launch for the kit's safe-area rules runs as v1's did.
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import { HOME_SCREEN_TAGS, STANDALONE_CHECK } from '@wisp/shell/pure';

const file = (p: string): string => fileURLToPath(new URL(`../${p}`, import.meta.url));
const pages = ['index.html', 'dist/index.html'].filter((p) => p === 'index.html' || existsSync(file(p)));
const meta = (html: string, name: string): string[] => [...html.matchAll(new RegExp(`<meta name="${name}"([^>]*)>`, 'g'))].map((m) => m[1] ?? '');

test('the_built_page_leaves_the_home_screen_tags_to_a_grid_box', () => {
  for (const p of pages) {
    const html = readFileSync(file(p), 'utf8');
    // v1 homescreen.js: the manifest and Apple's tags go in at runtime on a Grid only (wisp.grid-client, @wisp/shell
    // homeScreenTags), so a browser on someone's own computer is never offered to install wisp
    expect(html, p).not.toContain('rel="manifest"');
    for (const name of ['apple-mobile-web-app-capable', 'mobile-web-app-capable', 'apple-mobile-web-app-status-bar-style', 'apple-mobile-web-app-title', 'theme-color']) {
      expect(meta(html, name), `${p} ${name}`).toEqual([]);
    }
    // what the runtime adds points at the manifest this build ships
    expect(HOME_SCREEN_TAGS.find((t) => t.attrs['rel'] === 'manifest')?.attrs['href']).toBe('/manifest.webmanifest');
    expect(existsSync(file('public/manifest.webmanifest'))).toBe(true);
    // harmless everywhere, as v1's static head: the icon, the notch-aware viewport, the standalone mark
    expect(html, p).toContain('<link rel="apple-touch-icon" href="/apple-touch-icon.png" sizes="180x180" />');
    expect(html, p).toMatch(/<meta name="viewport" content="[^"]*viewport-fit=cover/);
    // the standalone check runs before any module (so the first frame is laid out for the notch)
    const at = html.indexOf(`<script>${STANDALONE_CHECK}</script>`);
    expect(at, p).toBeGreaterThan(0);
    expect(at, p).toBeLessThan(html.search(/<script type="module"/));
  }
});

test('the_manifest_installs_a_standalone_wisp_with_v1s_icons', () => {
  const m = JSON.parse(readFileSync(file('public/manifest.webmanifest'), 'utf8')) as Record<string, unknown> & { icons: { src: string; sizes: string; purpose?: string }[] };
  expect([m['name'], m['short_name'], m['display'], m['scope'], m['start_url'], m['id']]).toEqual(['wisp', 'wisp', 'standalone', '/', '/', '/']);
  expect([m['background_color'], m['theme_color']]).toEqual(['#ffffff', '#ffffff']);
  for (const size of ['192x192', '512x512']) expect(m.icons.some((i) => i.sizes === size && i.purpose === undefined), size).toBe(true);
  expect(m.icons.some((i) => i.purpose === 'maskable' && i.sizes === '512x512')).toBe(true);
  for (const i of m.icons) expect(existsSync(file(`public${i.src}`)), i.src).toBe(true);
});

test('the_standalone_check_marks_a_home_screen_launch_only', () => {
  const run = (o: { ua?: string; standalone?: boolean; mode?: boolean; capacitor?: boolean; android?: boolean }): boolean => {
    const attrs = new Map<string, string>();
    const ctx = {
      navigator: { userAgent: o.ua ?? 'Mozilla/5.0 (iPhone) Safari', standalone: o.standalone ?? false },
      window: { ...(o.capacitor !== undefined && { Capacitor: { isNativePlatform: () => o.capacitor } }), ...(o.android && { wispAndroid: {} }) },
      matchMedia: (q: string) => ({ matches: q === '(display-mode: standalone)' && (o.mode ?? false) }),
      document: { documentElement: { setAttribute: (k: string, v: string) => attrs.set(k, v) } },
    };
    new Function(...Object.keys(ctx), STANDALONE_CHECK)(...Object.values(ctx));
    return attrs.has('data-standalone');
  };
  expect(run({ standalone: true }), 'iOS home-screen app').toBe(true);
  expect(run({ mode: true }), 'installed from Chrome (display-mode: standalone)').toBe(true);
  expect(run({}), 'a browser tab').toBe(false);
  expect(run({ ua: 'Mozilla/5.0 wispMobile/1', standalone: true, mode: true }), 'the old Android app').toBe(false);
  expect(run({ capacitor: true, mode: true }), 'the native apps (Capacitor)').toBe(false);
  expect(run({ capacitor: false, mode: true }), 'Capacitor core in a browser').toBe(true);
  expect(run({ android: true, mode: true }), "the Android app's bridge").toBe(false);
});
