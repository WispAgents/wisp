// B53: eight of Settings' rows wrote keys no plugin declares (Babysitting mode, the editor, the worktree default,
// review, read times, the transcript default, notifications, the custom terminal command): the server refused every
// change ("no plugin declares the setting …") and the switch went back. Every key the panes write is declared.
import { expect, test } from 'vitest';
import { KEYS } from '../src/types.ts';

// every plugin's manifest, read as text (no node fs in a UI test)
const manifests = import.meta.glob<string>('../../../*/host/wisp-plugin.json', { query: '?raw', import: 'default', eager: true });

function declared(): Set<string> {
  const out = new Set<string>();
  for (const text of Object.values(manifests)) {
    const m = JSON.parse(text) as { contributes?: { settings?: { key: string }[] } };
    for (const s of m.contributes?.settings ?? []) out.add(s.key);
  }
  return out;
}

test('every setting key the Settings panes write is declared by a plugin manifest', () => {
  const have = declared();
  expect(Object.keys(manifests).length).toBeGreaterThan(20);
  // every key, Open tasks in's too (audit A F5: it wrote `terminal`, which no plugin declares, so no pick stuck; the
  // test skipped keys without a dot), and Notifications' switch (wisp.shell-home's notify.ts reads `settings.notify`)
  const keys = [...Object.values(KEYS), 'settings.notify'];
  expect(keys.filter((k) => !have.has(k))).toEqual([]);
});
