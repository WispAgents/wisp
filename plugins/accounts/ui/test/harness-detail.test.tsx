// @vitest-environment happy-dom
// Settings audit r1 review: the stale-model warning, its Reset and the update-check row were checked only by reading
// HarnessDetail.tsx as text. Rendered here: a saved model the harness no longer lists shows the warning, Reset writes
// runtime.launch back to the harness's own default (the other harnesses' untouched) and the warning goes; the shared
// rows draw the accounts.updateCheck switch as a row Settings' search can land on.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { signal, type WritableSignal } from '@wisp/client';
import { KitProvider } from '@wisp/ui';
import { Defaults, GlobalRows } from '../src/panes/HarnessDetail.tsx';
import type { AgentsData } from '../src/panes/agents-data.ts';
import type { HarnessRow } from '../src/types.ts';
import '../src/i18n/lazy.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

/** A client whose `setting` rows are signals and whose settings.set writes them (its optimistic patch, at once). */
function fakeClient(initial: Record<string, unknown>) {
  const rows = new Map<string, WritableSignal<{ key: string; value: unknown } | undefined>>();
  const row = (key: string) => rows.get(key) ?? (rows.set(key, signal<{ key: string; value: unknown } | undefined>(undefined)), rows.get(key)!);
  for (const [k, v] of Object.entries(initial)) row(`laptop:${k}`)({ key: k, value: v });
  const writes: { key: string; value: unknown }[] = [];
  const client = {
    home: 'laptop',
    entities: { row: (_kind: string, key: string) => row(key), list: () => signal([]) },
    run: async (_cmd: unknown, a: { key?: string; value?: unknown }) => {
      if (a.key !== undefined) {
        writes.push({ key: a.key, value: a.value });
        row(`laptop:${a.key}`)({ key: a.key, value: a.value });
      }
      return undefined;
    },
  };
  return { client, writes };
}

const mount = (view: () => import('solid-js').JSX.Element): HTMLElement => {
  const host = document.createElement('div');
  document.body.append(host);
  render(() => <KitProvider t={(k) => k}>{view()}</KitProvider>, host);
  return host;
};

const codex: HarnessRow = { id: 'codex', name: 'Codex', ord: 1, available: true, signIn: 'cli', models: [{ id: 'gpt-5', label: 'GPT-5' }] };
const none = (): void => undefined;

test('a_saved_model_the_harness_no_longer_lists_warns_and_reset_puts_back_its_own_settings_audit_r1', async () => {
  const launch = { agent: 'claude', models: { codex: 'gpt-4-retired', claude: 'opus' }, labels: { codex: 'GPT-4', claude: 'Opus' }, efforts: { claude: 'high' } };
  const { client, writes } = fakeClient({ 'runtime.launch': launch });
  const d = { c: client } as unknown as AgentsData;
  const host = mount(() => <Defaults d={d} h={() => codex} onKeys={none} onAdd={none} onRename={none} onRemove={none} />);
  const stale = (): Element | null => host.querySelector('[data-part="accounts.defaults.stale"]');
  expect(stale()?.textContent).toContain('gpt-4-retired');
  // the model row is the one Settings' search lands on for runtime.launch
  expect(host.querySelector('[data-key="runtime.launch"]')).not.toBeNull();
  const reset = [...(stale()?.querySelectorAll('button') ?? [])].find((b) => b.textContent?.includes('Reset'));
  expect(reset).toBeDefined();
  reset?.click();
  await Promise.resolve();
  expect(writes).toEqual([{ key: 'runtime.launch', value: { agent: 'claude', models: { claude: 'opus' }, labels: { claude: 'Opus' }, efforts: { claude: 'high' } } }]);
  expect(stale()).toBeNull();
});

test('a_model_it_still_lists_shows_no_warning_settings_audit_r1', () => {
  const { client } = fakeClient({ 'runtime.launch': { models: { codex: 'gpt-5' } } });
  const host = mount(() => <Defaults d={{ c: client } as unknown as AgentsData} h={() => codex} onKeys={none} onAdd={none} onRemove={none} onRename={none} />);
  expect(host.querySelector('[data-part="accounts.defaults.stale"]')).toBeNull();
});

test('the_update_check_is_a_row_settings_search_lands_on_settings_audit_r1', async () => {
  const { client, writes } = fakeClient({});
  const d = { c: client, peer: 'laptop', place: () => undefined } as unknown as AgentsData;
  const host = mount(() => <GlobalRows d={d} />);
  const row = host.querySelector<HTMLElement>('[data-key="accounts.updateCheck"]');
  expect(row).not.toBeNull();
  // on by default; switched off, it writes false
  const sw = row?.querySelector<HTMLElement>('[role="switch"], input[type="checkbox"]');
  expect(sw).not.toBeNull();
  sw?.click();
  await Promise.resolve();
  expect(writes).toEqual([{ key: 'accounts.updateCheck', value: false }]);
});
