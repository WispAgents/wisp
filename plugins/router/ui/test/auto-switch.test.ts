// Settings → Auto shows only while Auto's one switch is on (owner, 2026-11-17: off until it's reworked).
import { expect, test } from 'vitest';
import ui from '../src/index.ts';

test('settings_auto_pane_hides_while_autos_switch_is_off_owner_37', () => {
  const pane = ui.settings?.find((p) => p.id === 'auto');
  const rows = new Map<string, unknown>();
  const client = { home: 'home', entities: { row: (_e: string, k: string) => () => rows.get(k) } } as never;
  expect(pane?.if?.(client)).toBe(false);
  rows.set('home:runtime.auto', { value: true });
  expect(pane?.if?.(client)).toBe(true);
});
