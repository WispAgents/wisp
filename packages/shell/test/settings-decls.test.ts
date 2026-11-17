// Owner's desktop test 2026-11-17 #2: Settings → Theme never switched. `shell.theme` was a server key, and no
// manifest declares a `shell.*` key, so the server refused every write ("no plugin declares the setting
// shell.theme") and the radio went back. A shell key the server keeps must be one a manifest declares.
import { expect, test } from 'vitest';
import { SHELL_SETTINGS } from '../src/settings-decls.ts';

const manifests = import.meta.glob<string>('../../../plugins/*/host/wisp-plugin.json', { query: '?raw', import: 'default', eager: true });

function declared(): Set<string> {
  const out = new Set<string>();
  for (const text of Object.values(manifests)) {
    const m = JSON.parse(text) as { contributes?: { settings?: { key: string }[] } };
    for (const s of m.contributes?.settings ?? []) out.add(s.key);
  }
  return out;
}

test('every shell setting kept on the server is declared by a plugin manifest', () => {
  const have = declared();
  expect(Object.keys(manifests).length).toBeGreaterThan(20);
  const server = SHELL_SETTINGS.filter((d) => (d.scope as string) === 'server').map((d) => d.key);
  expect(server.filter((k) => !have.has(k))).toEqual([]);
});

test('the theme is a setting this device keeps (as the server treats it: crates/wisp/src/settings_store.rs)', () => {
  expect(SHELL_SETTINGS.find((d) => d.key === 'shell.theme')?.scope).toBe('device');
});
