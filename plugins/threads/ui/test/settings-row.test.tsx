// Settings → General's Check compactions (threads.compactAudit, ADR-0096): off when unset (its declared default), on
// when the row says true, a click writes the boolean through settings.set on the home wisp, a locked one says who.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'solid-js/web';
import { signal, type PluginClient, type SettingRow } from '@wisp/client';
import CompactAuditRow, { COMPACT_AUDIT } from '../src/CompactAuditRow.tsx';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});

function mount(value: SettingRow | undefined) {
  const row = signal<SettingRow | undefined>(value);
  const run = vi.fn(() => Promise.resolve(undefined));
  const client = { home: 'local', entities: { row: (kind: string, key: string) => (kind === 'setting' && key === `local:${COMPACT_AUDIT}` ? row : () => undefined) }, run } as unknown as PluginClient;
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <CompactAuditRow client={client} config={{}} />, host);
  return { host, run, row };
}
const sw = (host: HTMLElement): HTMLElement | null => host.querySelector('[role="switch"], input[type="checkbox"]');

test('unset reads off; the row has a plain label and its help', () => {
  const { host } = mount(undefined);
  expect(host.textContent).toContain('Check compactions');
  expect(host.textContent).toContain('After a compaction, wisp asks a copy of the conversation');
  expect(sw(host)?.getAttribute('aria-checked') ?? String((sw(host) as HTMLInputElement | null)?.checked)).toBe('false');
});

test('a click writes true through settings.set on the home wisp', () => {
  const { host, run } = mount({ key: COMPACT_AUDIT, value: false } as unknown as SettingRow);
  sw(host)?.click();
  expect(run).toHaveBeenCalledTimes(1);
  const [cmd, args] = run.mock.calls[0] as unknown as [{ id: string }, unknown];
  expect(cmd.id).toBe('settings.set');
  expect(args).toEqual({ peer: 'local', key: COMPACT_AUDIT, value: true });
});

test('a setting another plugin locked says which', () => {
  const { host } = mount({ key: COMPACT_AUDIT, value: true, locked_by: 'wisp.focus' } as unknown as SettingRow);
  expect(host.textContent).toContain('Set by the plugin wisp.focus');
});
