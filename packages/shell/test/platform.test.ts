// The platform module's look plumbing (ADR-0065 §5): the shell sets data-platform from platform.kind for the
// phone apps only; the browser never does, at any width. And the new calls (safeArea, windowLayout, openInApp,
// onBack) answer on every platform.
import { describe, expect, it } from 'vitest';
import { applyPlatformLook, platformLook } from '../src/platform/look.ts';
import { detectPlatform, nativeSeams, nullPlatform } from '../src/platform/index.ts';
import { loadPlatform } from '../src/platform/load.ts';
import type { BackEvent, CapacitorGlobal, PlatformWindow, Rect } from '../src/platform/types.ts';

interface Fake extends PlatformWindow {
  opened: string[];
  fire(type: string, detail?: unknown): void;
}

/** A window at `width` px; `coarse` is a touch screen; `standalone` a Home Screen web app. */
function fakeWindow(o: { width?: number; height?: number; coarse?: boolean; standalone?: boolean; ua?: string; cap?: CapacitorGlobal; tauri?: boolean; segments?: Rect[] } = {}): Fake {
  const width = o.width ?? 1440;
  const listeners = new Map<string, Set<(e: Event) => void>>();
  const opened: string[] = [];
  const media = (q: string): MediaQueryList => {
    const max = /max-width:\s*(\d+)px/.exec(q);
    const matches = q.includes('pointer: coarse') ? o.coarse === true : q.includes('display-mode: standalone') ? o.standalone === true : max ? width <= Number(max[1]) : false;
    return { matches, media: q, addEventListener: () => {}, removeEventListener: () => {} } as unknown as MediaQueryList;
  };
  return {
    navigator: { platform: o.ua ?? 'iPhone', userAgent: o.ua ?? 'iPhone' } as Navigator,
    document,
    matchMedia: media,
    addEventListener: (t, fn) => void (listeners.get(t) ?? listeners.set(t, new Set()).get(t))?.add(fn),
    removeEventListener: (t, fn) => void listeners.get(t)?.delete(fn),
    open: (url) => void opened.push(url),
    location: { reload: () => {}, origin: 'https://box.grid.trywisp.dev' },
    innerWidth: width,
    innerHeight: o.height ?? 844,
    ...(o.segments && { viewport: { segments: o.segments } }),
    ...(o.cap && { Capacitor: o.cap }),
    ...(o.tauri && { __TAURI_INTERNALS__: { invoke: () => Promise.resolve(null) } }),
    opened,
    fire(type, detail) {
      for (const fn of listeners.get(type) ?? []) fn(new CustomEvent(type, { detail }));
    },
  };
}

const native = (platform: 'android' | 'ios', plugins: CapacitorGlobal['Plugins'] = {}): CapacitorGlobal => ({ isNativePlatform: () => true, getPlatform: () => platform, Plugins: plugins });

function root(): Element {
  const el = document.createElement('html');
  return el;
}

// The entry loads the platform it runs on (ADR-0105's first screen): a browser's page carries only the browser's, the
// desktop app's, Android's and the iPhone's come with an import of their own; each is the one detectPlatform picks
describe('loadPlatform', () => {
  it('a_browser_loads_only_its_own_platform_and_the_apps_load_theirs', async () => {
    const cases: [PlatformWindow, string][] = [
      [fakeWindow({ width: 1440, ua: 'MacIntel' }), 'browser'],
      [fakeWindow({ width: 900, ua: 'MacIntel', tauri: true }), 'desktop'],
      [fakeWindow({ width: 390, coarse: true, cap: native('android') }), 'android'],
      [fakeWindow({ width: 390, coarse: true, cap: native('ios') }), 'ios'],
      [{ ...fakeWindow({ width: 390 }), wispAndroid: {} }, 'android'],
    ];
    for (const [w, kind] of cases) {
      expect((await loadPlatform(w)).kind).toBe(kind);
      expect(detectPlatform(w).kind).toBe(kind);
    }
  });
});

describe('data-platform', () => {
  it('a_browser_never_sets_data_platform', () => {
    // a phone's browser at 390 px, a Home Screen web app at 320 px, a desktop browser, the desktop app
    const browsers = [
      fakeWindow({ width: 390, coarse: true }),
      fakeWindow({ width: 320, coarse: true, standalone: true }),
      fakeWindow({ width: 1440, ua: 'MacIntel' }),
      fakeWindow({ width: 900, ua: 'MacIntel', tauri: true }),
    ];
    for (const w of browsers) {
      const p = detectPlatform(w);
      expect(['browser', 'desktop']).toContain(p.kind);
      const el = root();
      expect(applyPlatformLook(el, p)).toBeUndefined();
      expect(el.hasAttribute('data-platform')).toBe(false);
    }
    // a stale attribute (a page that was once an app's) is taken off
    const el = root();
    el.setAttribute('data-platform', 'ios');
    applyPlatformLook(el, nullPlatform());
    expect(el.hasAttribute('data-platform')).toBe(false);
  });

  it('the phone apps set their look', () => {
    for (const kind of ['android', 'ios'] as const) {
      const p = detectPlatform(fakeWindow({ width: 390, coarse: true, cap: native(kind) }));
      expect(p.kind).toBe(kind);
      expect(p.phone).toBe(true);
      expect(platformLook(p)).toBe(kind);
      const el = root();
      expect(applyPlatformLook(el, p)).toBe(kind);
      expect(el.getAttribute('data-platform')).toBe(kind);
    }
    // MainActivity's bridge without Capacitor is still the Android app
    const w = fakeWindow({ width: 390 });
    expect(detectPlatform({ ...w, wispAndroid: {} }).kind).toBe('android');
  });

  it('only the phone apps are phones: a browser at phone width is not', () => {
    expect(detectPlatform(fakeWindow({ width: 390, coarse: true })).phone).toBe(false);
    expect(detectPlatform(fakeWindow({ tauri: true })).phone).toBe(false);
    expect(nullPlatform().phone).toBe(false);
  });
});

describe('the new platform calls', () => {
  it('windowLayout gives the M3 size class and a foldable hinge', () => {
    expect(detectPlatform(fakeWindow({ width: 390 })).windowLayout()).toEqual({ width: 390, height: 844, size: 'compact' });
    expect(detectPlatform(fakeWindow({ width: 600 })).windowLayout().size).toBe('medium');
    expect(detectPlatform(fakeWindow({ width: 839 })).windowLayout().size).toBe('medium');
    expect(detectPlatform(fakeWindow({ width: 840 })).windowLayout().size).toBe('expanded');
    // two screens side by side, a 20 px gap between them
    const fold = fakeWindow({ width: 860, height: 700, cap: native('android'), segments: [{ x: 0, y: 0, width: 420, height: 700 }, { x: 440, y: 0, width: 420, height: 700 }] });
    expect(detectPlatform(fold).windowLayout().hinge).toEqual({ x: 420, y: 0, width: 20, height: 700 });
    // stacked (a fold held flat, hinge across)
    const book = fakeWindow({ width: 700, height: 860, segments: [{ x: 0, y: 0, width: 700, height: 420 }, { x: 0, y: 440, width: 700, height: 420 }] });
    expect(detectPlatform(book).windowLayout().hinge).toEqual({ x: 0, y: 420, width: 700, height: 20 });
  });

  it('safeArea answers numbers everywhere (zero with no insets)', () => {
    for (const w of [fakeWindow(), fakeWindow({ cap: native('ios') }), fakeWindow({ tauri: true })]) {
      expect(detectPlatform(w).safeArea()).toEqual({ top: 0, right: 0, bottom: 0, left: 0 });
    }
    expect(nullPlatform().safeArea()).toEqual({ top: 0, right: 0, bottom: 0, left: 0 });
  });

  it('openInApp uses the in-app browser in the apps, a tab in the browser', async () => {
    const browser = fakeWindow();
    detectPlatform(browser).openInApp('https://example.com/a');
    expect(browser.opened).toEqual(['https://example.com/a']);
    const shown: unknown[] = [];
    const app = fakeWindow({ cap: native('ios', { Browser: { open: async (a) => void shown.push(a) } }) });
    detectPlatform(app).openInApp('https://example.com/b');
    expect(shown).toEqual([{ url: 'https://example.com/b' }]);
    expect(app.opened).toEqual([]);
    // an app build without the Browser plugin still opens it (outside)
    const old = fakeWindow({ cap: native('android') });
    detectPlatform(old).openInApp('https://example.com/c');
    await Promise.resolve();
    await Promise.resolve();
    expect(old.opened).toEqual(['https://example.com/c']);
  });

  // the owner (2026-11-19): links from the desktop app didn't reach the browser. The shell sends a window.open out
  // (desktop/src-tauri main.rs on_new_window); the page's opener IPC is refused (capability.json), and a window.open
  // made only after that refusal comes after the click, where a webview may hold it as an unasked popup.
  it('the desktop app opens a link out in the click itself, never over the opener IPC', () => {
    const w = fakeWindow({ ua: 'MacIntel' });
    const asked: string[] = [];
    const tauri = { invoke: (cmd: string) => (asked.push(cmd), Promise.reject(new Error(`${cmd} not allowed`))) };
    const p = detectPlatform({ ...w, __TAURI_INTERNALS__: tauri });
    expect(p.kind).toBe('desktop');
    p.openExternal('https://github.com/WispAgents/wisp/pull/1');
    p.openInApp('https://example.com/preview');
    expect(w.opened).toEqual(['https://github.com/WispAgents/wisp/pull/1', 'https://example.com/preview']);
    expect(asked.filter((c) => c.startsWith('plugin:opener'))).toEqual([]);
  });

  it('onBack passes the native gesture through in the apps and never fires in a browser', () => {
    const w = fakeWindow({ cap: native('android') });
    const seen: BackEvent[] = [];
    const stop = detectPlatform(w).onBack((e) => seen.push(e));
    w.fire('wisp-back', { phase: 'start', edge: 'left' });
    w.fire('wisp-back', { phase: 'progress', edge: 'left', progress: 0.4 });
    w.fire('wisp-back', { phase: 'progress', edge: 'left', progress: 7 }); // clamped
    w.fire('wisp-back', { phase: 'nonsense' }); // ignored
    w.fire('wisp-back', { phase: 'commit' });
    stop();
    w.fire('wisp-back', { phase: 'cancel' });
    expect(seen).toEqual([{ phase: 'start', edge: 'left' }, { phase: 'progress', edge: 'left', progress: 0.4 }, { phase: 'progress', edge: 'left', progress: 1 }, { phase: 'commit' }]);

    const b = fakeWindow();
    const none: BackEvent[] = [];
    detectPlatform(b).onBack((e) => none.push(e));
    b.fire('wisp-back', { phase: 'commit' });
    expect(none).toEqual([]);
  });
});

describe('the phone apps’ native seams (ADR-0065 step 8)', () => {
  const cap = (methods: Record<string, (args?: unknown) => Promise<unknown>>): CapacitorGlobal => ({ isNativePlatform: () => true, Plugins: { WispSsh: methods } });

  it('the_native_seams_answer_unsupported_until_the_native_side_has_them', async () => {
    const refuse = async (): Promise<never> => {
      throw new Error('not implemented');
    };
    const p = nativeSeams(cap({ secureGet: refuse, secureSet: refuse, secureRemove: refuse, scanCode: refuse, registerPush: refuse }), 'android');
    expect(await p.secureStore.get('k')).toBeNull();
    expect(await p.secureStore.set('k', 'v')).toBe('unsupported');
    await expect(p.secureStore.remove('k')).resolves.toBeUndefined();
    expect(await p.scanCode?.()).toBe('unsupported');
    expect(await p.registerPush()).toBe('unsupported');
    expect(nativeSeams(undefined, 'ios').scanCode, 'nor a WebView without the native side: no scanner at all').toBeUndefined();
    const web = detectPlatform(fakeWindow());
    expect(web.secureStore ?? web.scanCode ?? web.registerPush, 'a browser has none').toBeUndefined();
  });

  it('the_native_seams_pass_what_the_native_side_answers', async () => {
    const vault = new Map<string, string>();
    const key = (a: unknown): string => (a as { key: string }).key;
    const p = nativeSeams(
      cap({
        secureGet: async (a) => ({ value: vault.get(key(a)) ?? null }),
        secureSet: async (a) => {
          vault.set(key(a), (a as { value: string }).value);
          return {};
        },
        secureRemove: async (a) => {
          vault.delete(key(a));
          return {};
        },
        scanCode: async () => ({ text: 'wisp+ssh://me@studio?fp=x' }),
        registerPush: async () => ({ handle: 'h1', publicKey: 'pk' }),
      }),
      'ios',
    );
    expect(await p.secureStore.set('wisp.k', 'sealed')).toBe('set');
    expect(await p.secureStore.get('wisp.k')).toBe('sealed');
    await p.secureStore.remove('wisp.k');
    expect(await p.secureStore.get('wisp.k')).toBeNull();
    expect(await p.scanCode?.()).toEqual({ text: 'wisp+ssh://me@studio?fp=x' });
    expect(await p.registerPush()).toEqual({ handle: 'h1', publicKey: 'pk', platform: 'ios' });
  });

  // the Android app had no scanCode, yet the seam was always there: Add computer's main button always ended in "type it
  // by hand". It's there only when the plugin has the method, and it reads each answer the scanner gives
  it('the_scanner_is_there_only_when_the_native_side_has_it_and_reads_its_answers', async () => {
    expect(nativeSeams(cap({ secureGet: async () => ({}) }), 'android').scanCode, 'an app without it').toBeUndefined();
    const answer = (r: unknown) => nativeSeams(cap({ scanCode: async () => r }), 'android').scanCode?.();
    expect(await answer({ denied: true })).toBe('denied');
    expect(await answer({})).toBe('cancelled');
    expect(await answer({ cancelled: true })).toBe('cancelled');
    expect(await answer({ text: '' })).toBe('cancelled');
    expect(await answer({ text: 'wisp+ssh://me@studio?fp=x' })).toEqual({ text: 'wisp+ssh://me@studio?fp=x' });
    const rejected = nativeSeams(cap({ scanCode: () => Promise.reject(new Error('no')) }), 'android');
    expect(await rejected.scanCode?.()).toBe('unsupported');
  });
});

// Android's Share had no way out (no Share plugin in the build, no navigator.share in the System WebView): every Share
// was a silent copy. The app's own chooser (WispSsh.share) comes first; an app without it falls back as before
describe('share in the Android app', () => {
  it('the_app_s_own_share_sheet_takes_a_share', async () => {
    const asked: unknown[] = [];
    const w = fakeWindow({ cap: native('android', { WispSsh: { share: async (a) => (asked.push(a), { shared: true }) } }) });
    expect(await detectPlatform(w).share({ title: 'PR', text: 'Look', url: 'https://github.com/x/1' })).toBe('shared');
    expect(asked).toEqual([{ title: 'PR', text: 'Look', url: 'https://github.com/x/1' }]);
    const told = fakeWindow({ cap: native('android', { WispSsh: { share: async () => ({ cancelled: true }) } }) });
    expect(await detectPlatform(told).share({ text: 'x' })).toBe('cancelled');
    // without it (or an older app refusing it): the browser's answer, as before
    const old = fakeWindow({ cap: native('android', { WispSsh: { share: () => Promise.reject(new Error('not implemented')) } }) });
    expect(await detectPlatform(old).share({ text: 'x' })).toBe('unsupported');
    expect(await detectPlatform(fakeWindow({ cap: native('android') })).share({ text: 'x' })).toBe('unsupported');
  });
});
