/// <reference types="vite/client" />
// The shell's lazily used strings (en.lazy.json, ADR-0011) load with the code that uses them, never with the first page:
// App.tsx imported i18n/lazy.ts for one key (the side sheet's title) and the whole lazy catalogue, 23 KB, joined the
// entry chunk (initial JS 124 KB br over its 120 budget). A key the first page uses is in en.ts.
import { expect, test } from 'vitest';
import app from '../src/frame/App.tsx?raw';
import lazy from '../src/i18n/en.lazy.json' with { type: 'json' };
import { SHELL_STRINGS } from '../src/i18n/en.ts';

test('the_frame_never_pulls_in_the_lazy_catalogue_and_its_strings_are_eager', () => {
  expect(app).not.toMatch(/import\s+'\.\.\/i18n\/lazy\.ts'/);
  const keys = [...app.matchAll(/\bt\(\s*['"`](shell\.[A-Za-z0-9_.]+)['"`]/g)].map((m) => m[1] as string);
  expect(keys.length).toBeGreaterThan(0);
  for (const k of keys) {
    expect(Object.hasOwn(lazy, k), `${k} is a first-page string: en.ts, not en.lazy.json`).toBe(false);
    expect(Object.hasOwn(SHELL_STRINGS, k), `${k} in en.ts`).toBe(true);
  }
});
