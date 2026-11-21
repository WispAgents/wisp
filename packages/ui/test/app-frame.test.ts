// ADR-0065 edge case: a My Stuff app in the phone app gets the platform tokens as CSS variables, so it can
// match Material 3 or HIG; in the browser it gets wisp's tokens only, as before.
import { describe, expect, it } from 'vitest';
import { FRAME_TOKENS, PLATFORM_FRAME_TOKENS, readTheme, themeCss } from '../src/frame/theme.ts';
import { lookOf } from '../src/platform/look.ts';

/** A page whose computed style answers `vars`, with `data-platform` as given. */
function page(vars: Record<string, string>, platform?: string): Document {
  const attrs: Record<string, string> = platform ? { 'data-platform': platform } : {};
  const root = { getAttribute: (n: string) => attrs[n] ?? null };
  const view = {
    getComputedStyle: () => ({ getPropertyValue: (k: string) => vars[k] ?? '' }),
    matchMedia: () => ({ matches: false }),
  };
  return { documentElement: root, defaultView: view } as unknown as Document;
}

const ALL: Record<string, string> = {
  '--ink': '#171717',
  '--accent': '#2f5bea',
  '--surface-c0': '#ffffff',
  '--on-surface': '#000000',
  '--tap-min': '44px',
  '--font-ui': 'system-ui',
  '--safe-bottom': '34px',
  '--radius-md': '10px',
};

describe('app frame tokens', () => {
  it('an_app_frame_gets_the_platform_tokens', () => {
    const th = readTheme(page(ALL, 'ios'));
    expect(th.vars['--tap-min']).toBe('44px');
    expect(th.vars['--surface-c0']).toBe('#ffffff');
    expect(th.vars['--on-surface']).toBe('#000000');
    expect(th.vars['--safe-bottom']).toBeUndefined(); // the frame's chrome is inside the safe area already
    expect(th.vars['--ink']).toBe('#171717'); // wisp's own still come
    const css = themeCss(th.vars, th.scheme);
    expect(css).toContain('--tap-min:44px');
    expect(css).toContain('--font-ui:system-ui');
  });

  it('a browser frame gets wisp tokens only, at any width', () => {
    const th = readTheme(page(ALL));
    expect(Object.keys(th.vars).every((k) => (FRAME_TOKENS as readonly string[]).includes(k))).toBe(true);
    expect(th.vars['--tap-min']).toBeUndefined();
    expect(readTheme(page(ALL, 'desktop')).vars['--tap-min']).toBeUndefined();
  });

  it('the platform list covers §A.8 roles a framed page can use and stays disjoint from the stable list', () => {
    for (const k of ['--tap-min', '--surface-c0', '--surface-c4', '--on-surface', '--on-surface-var', '--outline', '--primary', '--font-ui', '--radius-sm', '--radius-lg'] as const) {
      expect(PLATFORM_FRAME_TOKENS as readonly string[]).toContain(k);
    }
    expect((PLATFORM_FRAME_TOKENS as readonly string[]).some((k) => (FRAME_TOKENS as readonly string[]).includes(k))).toBe(false);
  });

  it('only android and ios are a phone look', () => {
    expect(lookOf('android')).toBe('android');
    expect(lookOf('ios')).toBe('ios');
    for (const v of [null, undefined, '', 'browser', 'desktop', 'IOS']) expect(lookOf(v)).toBe('web');
  });
});
