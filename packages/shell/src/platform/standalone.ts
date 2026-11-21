// F-phone-6 (v1 index.html's first script, homescreen.js): wisp opened from an iPhone's Home Screen (or installed
// from Chrome) runs full screen, so the kit keeps the page out of the status bar, the notch and the home indicator
// (app-stack.css, html[data-standalone]). Set before any module, so the first frame is already laid out for it. A
// browser tab never gets it, nor the native apps (Capacitor's bridge, the Android app's `wispAndroid` and its
// `wispMobile` user agent): they lay out their own edges.

/** index.html's inline script, as a string (the test runs this very text). */
export const STANDALONE_CHECK = `try{var n=navigator,w=window;var c=w.Capacitor;if(!(c&&c.isNativePlatform&&c.isNativePlatform())&&!w.wispAndroid&&!/\\bwispMobile\\b/.test(n.userAgent)&&(n.standalone===true||matchMedia('(display-mode: standalone)').matches))document.documentElement.setAttribute('data-standalone','')}catch(e){}`;

// F-phone-6 on a Grid only (v1 homescreen.js homeScreenTags): the manifest and Apple's Home Screen tags go into the
// head at runtime once the page knows it's served by a grid box (wisp.grid-client's row carries the box's preview
// template, ADR-0064), so a browser on someone's own computer is never offered to install wisp, and the native apps
// (which lay out their own edges and install nothing) are left alone. Added before the user can reach Share → Add to
// Home Screen: the row lands with the first sync.

/** One head tag: its element and attributes, in order. */
export interface HomeScreenTag {
  readonly tag: 'link' | 'meta';
  readonly attrs: Readonly<Record<string, string>>;
}

/** What iOS and Chrome read to install wisp, and the status bar's colour per scheme (the kit's --bg, v1's values). */
export const HOME_SCREEN_TAGS: readonly HomeScreenTag[] = [
  { tag: 'link', attrs: { rel: 'manifest', href: '/manifest.webmanifest' } },
  { tag: 'meta', attrs: { name: 'apple-mobile-web-app-capable', content: 'yes' } },
  { tag: 'meta', attrs: { name: 'mobile-web-app-capable', content: 'yes' } },
  { tag: 'meta', attrs: { name: 'apple-mobile-web-app-status-bar-style', content: 'black-translucent' } },
  { tag: 'meta', attrs: { name: 'apple-mobile-web-app-title', content: 'wisp' } },
  { tag: 'meta', attrs: { name: 'theme-color', media: '(prefers-color-scheme: light)', content: '#ffffff' } },
  { tag: 'meta', attrs: { name: 'theme-color', media: '(prefers-color-scheme: dark)', content: '#111111' } },
];

/** The desktop app, the Android or iPhone app (Capacitor, `wispAndroid`) or the old Android app (`wispMobile`). */
export function nativeShell(w: {
  readonly navigator?: { readonly userAgent?: string };
  readonly __TAURI_INTERNALS__?: unknown;
  readonly Capacitor?: { isNativePlatform?(): boolean };
  readonly wispAndroid?: unknown;
}): boolean {
  return w.__TAURI_INTERNALS__ !== undefined || w.Capacitor?.isNativePlatform?.() === true || w.wispAndroid !== undefined || /\bwispMobile\b/.test(w.navigator?.userAgent ?? '');
}

/** The decision: the tags in a grid box's browser page, none anywhere else. */
export const homeScreenTags = (o: { readonly inBox: boolean; readonly native: boolean }): readonly HomeScreenTag[] =>
  o.inBox && !o.native ? HOME_SCREEN_TAGS : [];

/** Appends `tags` to the head once (a page that already has a manifest link is left as it is). */
export function addHomeScreenTags(doc: Pick<Document, 'head' | 'createElement' | 'querySelector'>, tags: readonly HomeScreenTag[]): boolean {
  if (tags.length === 0 || doc.querySelector('link[rel="manifest"]')) return false;
  for (const t of tags) {
    const n = doc.createElement(t.tag);
    for (const [k, v] of Object.entries(t.attrs)) n.setAttribute(k, v);
    doc.head.append(n);
  }
  return true;
}
