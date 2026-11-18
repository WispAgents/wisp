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

// The first screen's lazy chunks (the row menu, the phone header, its tabs, bulk actions) load with Home, so their
// strings are eager and they never pull en.lazy.json, 6.5 KB br, into page 1 (ADR-0105). Keys built at run time count:
// the phone menu's section heads and submenus (task-menu.ts phoneMenu) are `shell.menu.section.*` and `shell.menu.*`.
const firstScreen = import.meta.glob(['../src/task-menu.ts', '../src/frame/{PhoneHead,PhoneNav}.tsx', '../src/frame/{destinations,bulk}.ts'], { query: '?raw', import: 'default', eager: true }) as Record<string, string>;

test('the_first_screens_lazy_chunks_use_only_eager_strings', () => {
  expect(Object.keys(firstScreen)).toHaveLength(5);
  const built = ['task', 'waiting', 'look', 'keep', 'place'].map((s) => `shell.menu.section.${s}`).concat(['view', 'goal', 'copy', 'files'].map((s) => `shell.menu.${s}`));
  for (const [file, src] of Object.entries(firstScreen)) {
    expect(src, file).not.toMatch(/import\s+'(\.\.\/|\.\/)i18n\/lazy\.ts'/);
    const keys = [...src.matchAll(/['"`](shell\.[\w.]+)['"`]/g)].map((m) => m[1] as string);
    for (const k of file.endsWith('task-menu.ts') ? [...keys, ...built] : keys) {
      const eager = Object.hasOwn(SHELL_STRINGS, k) || Object.hasOwn(SHELL_STRINGS, `${k}.one`);
      expect(eager, `${file}: ${k} in en.ts`).toBe(true);
      expect(Object.hasOwn(lazy, k), `${file}: ${k} not in en.lazy.json`).toBe(false);
    }
  }
});
