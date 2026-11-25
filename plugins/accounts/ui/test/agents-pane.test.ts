// Settings → Agents' rules (words.ts): a missing harness's one-line install, and what Re-check turned up
// (v1 settings.js paintAgents, setup.js setupCommand, recheckSummary).
import { expect, test } from 'vitest';
import type { DetectRow, HarnessRow } from '../src/types.ts';
import { harnessLine, harnessState, howToGet, mergeHarnesses, recheckSummary, toolState, updateAllSummary } from '../src/words.ts';
import '../src/i18n/lazy.ts';
import { recheck, setPluginEnabled } from '../src/commands.ts';
import { defaultsOf, fullest, harnessFilter, holdOf, pickOf, pluginOf, staleModel, STATUS_TONE, statusOf, withDisabled, withEffort, withHold, withModel } from '../src/harness-pane.ts';
import { PANE_KEYS } from '../src/pane-keys.ts';
import { listStep } from '@wisp/ui/pure';
// the detail's source, as text: which setting rows it has (rendering it needs the shell)
import DETAIL from '../src/panes/HarnessDetail.tsx?raw';
import { createRoot } from 'solid-js';
import { signal } from '@wisp/client';
import { useDetected } from '../src/detected.ts';

const h = (id: string, more: Partial<HarnessRow> = {}): HarnessRow => ({ id, name: id, ord: 1, available: true, models: [], signIn: 'cli', ...more });

test('a_missing_harness_shows_the_one_line_install_for_this_os', () => {
  const setup = { tip: 'Install Claude Code and run claude once to sign in', command: 'curl -fsSL https://claude.ai/install.sh | bash', url: 'https://claude.com/claude-code' };
  // found: nothing to install
  expect(howToGet(h('claude', { setup }))).toBeUndefined();
  // signed out is still found: it signs in, it doesn't install
  expect(howToGet(h('claude', { available: false, signedIn: false, setup }))).toBeUndefined();
  // not found: the OS's tip and its command to copy
  expect(howToGet(h('claude', { available: false, setup }))).toEqual({ tip: setup.tip, command: setup.command });
  // wisp installs it itself (opencode): Install, no command to copy; while it installs, the row says so instead
  expect(howToGet(h('opencode', { available: false, installable: true, setup }))).toEqual({ tip: setup.tip });
  expect(howToGet(h('opencode', { available: false, installable: true, installing: true, setup }))).toBeUndefined();
  // no tip from the harness: v1's words
  expect(howToGet(h('grok', { available: false, name: 'Grok' }))?.tip).toBe('Install the Grok CLI and sign in, then re-check');
});

test('recheck_finds_harnesses_and_models_again_and_says_what_changed', () => {
  const m = (n: number) => Array.from({ length: n }, (_, i) => ({ id: `m${i}`, label: `M${i}` }));
  const before = new Map([
    ['claude', { available: true, models: m(2), version: '1.0' }],
    ['codex', { available: false, models: [], version: undefined }],
    ['grok', { available: true, models: m(1), version: '1' }],
  ] as const);
  const after = [h('claude', { name: 'Claude Code', models: m(3), version: '1.0' }), h('codex', { name: 'Codex' }), h('grok', { name: 'Grok', available: false })];
  expect(recheckSummary(before as never, after)).toBe('Claude Code +1 model · Codex found · Grok not found');
  expect(recheckSummary(new Map([['claude', { available: true, models: m(3), version: '1.0' }]]) as never, [after[0] as HarnessRow])).toBe('Up to date: nothing new');
});

test('recheck_sends_no_arguments_the_server_reads_as_none', async () => {
  // `accounts.recheck` takes `()`: `{ peer }` was refused, so Re-check never ran
  expect((recheck as unknown as { wire: (a: unknown) => unknown }).wire({ peer: 'p1' })).toBeNull();
});

// ADR-0066: Settings → Agents lists the harnesses from wisp.runtime's detection rows, so a row shows before any CLI
// answers ("Checking…", no install tip yet) and settles in place with this plugin's sign-in and accounts.
test('settings_agents_shows_each_harness_checking_until_its_detect_lands', () => {
  const d = (id: string, ord: number, state: DetectRow['state'], agent: Partial<DetectRow['agent']> = {}): DetectRow => ({ id, ord, state, agent: { name: id, available: false, models: [], ...agent } });
  // nothing from wisp.accounts yet: every harness, in order, checking
  const first = mergeHarnesses([], [d('codex', 1, 'checking'), d('claude', 0, 'checking')]);
  expect(first.map((h) => [h.id, harnessState(h).label])).toEqual([
    ['claude', 'Checking…'],
    ['codex', 'Checking…'],
  ]);
  expect(harnessState(first[0] as HarnessRow).tone).toBe('neutral');
  // never seen: no install tip, no "Not found" before its answer
  expect(howToGet({ ...(first[0] as HarnessRow), setup: { tip: 'Install it' } })).toBeUndefined();
  expect(harnessLine(first[0] as HarnessRow, 0)).toBe('Looking for it on this computer');
  // claude lands (with its sign-in kind and brand from wisp.accounts); codex still checks, holding back only itself
  const mine = [h('claude', { ord: 0, available: false, signIn: 'cli', brand: 'Claude' })];
  const next = mergeHarnesses(mine, [d('claude', 0, 'installed', { available: true, version: '2.1.30', models: [{ id: 'opus', label: 'Opus' }] }), d('codex', 1, 'checking')]);
  expect(next.map((x) => [x.id, harnessState(x).label])).toEqual([
    ['claude', 'Ready'],
    ['codex', 'Checking…'],
  ]);
  expect(next[0]).toMatchObject({ signIn: 'cli', brand: 'Claude', version: '2.1.30', available: true });
  expect(next[0]?.checking).toBeUndefined();
  expect(harnessLine(next[0] as HarnessRow, 0)).toBe('2.1.30 · 1 model');
  // a Re-check: the last answer stays on screen while it checks again
  const again = mergeHarnesses(next, [d('claude', 0, 'checking', { available: true, version: '2.1.30', models: [{ id: 'opus', label: 'Opus' }] })]);
  expect(harnessLine(again[0] as HarnessRow, 0)).toBe('2.1.30 · 1 model');
  // missing: its tip again
  const gone = mergeHarnesses([], [d('codex', 1, 'missing', { setup: { tip: 'npm i -g codex' } })]);
  expect(harnessState(gone[0] as HarnessRow).label).toBe('Not found');
  expect(howToGet(gone[0] as HarnessRow)?.tip).toBe('npm i -g codex');
  // a wisp before the topic: wisp.accounts' rows as they were
  expect(mergeHarnesses([h('b', { ord: 2 }), h('a', { ord: 1 })], []).map((x) => x.id)).toEqual(['a', 'b']);
});

// round 324: the pane's harness rows follow each detect row as it settles (same key, the row changes in place); a
// createMemo over the keys read each row once, so a harness stayed "Checking…" until the pane opened again
test('settings_agents_follows_a_detect_row_that_settles_after_its_key_showed', () => {
  const d = (id: string, state: DetectRow['state'], available = false): DetectRow => ({ id, ord: 0, state, agent: { name: id, available, models: [] } });
  const keys = signal<readonly string[]>([]);
  const rows = new Map<string, ReturnType<typeof signal<DetectRow | undefined>>>();
  const at = (k: string) => rows.get(k) ?? (rows.set(k, signal<DetectRow | undefined>(undefined)), rows.get(k)!);
  const entities = { keys: () => keys, row: (_kind: string, k: string) => at(k) };
  createRoot((dispose) => {
    const list = useDetected(entities as never, 'laptop');
    const words = (): string[] => mergeHarnesses([], list()).map((h) => `${h.id}:${harnessState(h).label}`);
    at('laptop:claude')(d('claude', 'checking'));
    at('box:codex')(d('codex', 'installed', true));
    keys(['laptop:claude', 'box:codex']);
    expect(words(), 'this wisp’s only, checking').toEqual(['claude:Checking…']);
    at('laptop:claude')(d('claude', 'installed', true));
    expect(words(), 'its detect landed: one row, settled').toHaveLength(1);
    expect(words()[0], 'no longer checking').not.toBe('claude:Checking…');
    expect(list()[0]?.state, 'the row as it is now').toBe('installed');
    at('laptop:claude')(undefined);
    expect(words(), 'gone').toEqual([]);
    dispose();
  });
});

// QA r1 #23: Antigravity read "Sign in" beside "● Ready" (the row said Ready from its detect, the button Sign in from
// its CLI's own login check), and the GitHub row said nothing of gh's sign-in
test('a_harness_its_cli_says_is_signed_out_reads_signed_out_and_github_says_its_sign_in', () => {
  const agy = h('antigravity', { available: true, signIn: 'cli' });
  expect(harnessState(agy).label).toBe(harnessState(h('x', { available: true })).label);
  expect(harnessState(agy, false)).toEqual(harnessState(h('y', { available: false, signedIn: false })));
  expect(harnessState(agy, true)).toEqual(harnessState(h('z', { available: true })));
  expect(toolState({ id: 'gh', name: 'GitHub CLI', installed: true, signedIn: true })?.tone).toBe('good');
  expect(toolState({ id: 'gh', name: 'GitHub CLI', installed: true, signedIn: false })?.tone).toBe('warn');
  expect(toolState({ id: 'gh', name: 'GitHub CLI', installed: true })).toBeUndefined();
  expect(toolState({ id: 'gh', name: 'GitHub CLI', installed: false })?.tone).toBe('bad');
});

// Audit I IS11: Update all ran three updates that didn't take and said nothing; one Update says why. It says, for each,
// what it went to or why not (the server's words name the command to run), and how many took.
test('update_all_says_what_took_and_what_didnt_audit_i_is11', () => {
  const said = updateAllSummary([
    { name: 'Codex', version: '9.9.10' },
    { name: 'Claude Code', error: 'Claude Code is still 9.9.9 after the update (the latest is 9.9.10). Run `/x/claude update` in a terminal, then re-check.' },
    { name: 'opencode', error: 'Updating opencode failed: exit 1' },
  ]);
  expect(said.failed).toBe(true);
  expect(said.message).toBe('Update all: 1 of 3 updated');
  expect(said.description.split('\n')).toEqual([
    '✓ Codex 9.9.10',
    '✗ Claude Code is still 9.9.9 after the update (the latest is 9.9.10). Run `/x/claude update` in a terminal, then re-check.',
    '✗ Updating opencode failed: exit 1',
  ]);
  expect(updateAllSummary([{ name: 'Codex', version: '9.9.10' }, { name: 'Grok', version: '1.2' }]).failed).toBe(false);
});

// ---------- Settings audit r1: Settings → Agents as a list and the harness picked ----------

test('the_list_dot_says_ready_signed_out_missing_checking_or_disabled_settings_audit_r1', () => {
  expect(statusOf(h('a', { available: true }))).toBe('ready');
  // its CLI's own login says signed out: the dot follows it, as the row's Sign in does
  expect(statusOf(h('a', { available: true }), false)).toBe('signedOut');
  expect(statusOf(h('a', { available: false, signedIn: false }))).toBe('signedOut');
  expect(statusOf(h('a', { available: false }))).toBe('missing');
  expect(statusOf(h('a', { available: false, checking: true }))).toBe('checking');
  // its plugin off: Disabled, whatever its last row said
  expect(statusOf(h('a', { available: true, disabled: true }))).toBe('disabled');
  expect(STATUS_TONE.ready).toBe('good');
  expect(STATUS_TONE.signedOut).toBe('warn');
  expect(STATUS_TONE.missing).toBe('bad');
  expect(harnessState(h('a', { available: true, disabled: true })).label).toBe('Disabled');
});

test('a_harness_whose_plugin_is_off_stays_listed_and_switches_back_on_settings_audit_r1', () => {
  const known = [
    { id: 'claude', name: 'Claude Code', order: 10 },
    { id: 'grok', name: 'Grok', order: 60 },
  ];
  const plugins = [
    { id: 'wisp.claude', enabled: true },
    { id: 'wisp.grok', enabled: false },
  ];
  // runtime lists only the harnesses that are on: Grok comes back from its registered mark, Disabled
  const rows = withDisabled([h('claude', { available: true })], known, plugins);
  expect(rows.map((r) => [r.id, statusOf(r)])).toEqual([
    ['claude', 'ready'],
    ['grok', 'disabled'],
  ]);
  expect(rows[1]?.name).toBe('Grok');
  // a row runtime still has is marked too, and nothing is added twice
  expect(withDisabled([h('grok', { available: true })], known, plugins).map((r) => [r.id, r.disabled])).toEqual([['grok', true]]);
  // Advanced → Enabled turns its plugin back on through plugins.setEnabled
  expect(pluginOf('grok', plugins)?.id).toBe('wisp.grok');
  const cmd = setPluginEnabled as unknown as { id: string; wire: (a: unknown) => unknown };
  expect(cmd.id).toBe('plugins.setEnabled');
  expect(cmd.wire({ peer: 'p', id: 'wisp.grok', enabled: true })).toEqual({ id: 'wisp.grok', enabled: true });
});

test('a_defaults_write_changes_only_that_harnesss_model_settings_audit_r1', () => {
  const launch = { agent: 'codex', models: { claude: 'opus', codex: 'gpt-5' }, labels: { claude: 'Opus', codex: 'GPT-5' }, efforts: { claude: 'max', codex: 'high' } };
  const next = withModel(launch, 'claude', 'sonnet', 'Sonnet');
  expect(next.models).toEqual({ claude: 'sonnet', codex: 'gpt-5' });
  expect(next.labels).toEqual({ claude: 'Sonnet', codex: 'GPT-5' });
  // the others' keys, every level and New task's last harness as they were
  expect(next.efforts).toEqual(launch.efforts);
  expect(next.agent).toBe('codex');
  expect(defaultsOf(next, 'claude')).toEqual({ model: 'sonnet', effort: 'max' });
  // a level: only its own
  expect(withEffort(launch, 'codex', 'low').efforts).toEqual({ claude: 'max', codex: 'low' });
  expect(withEffort(launch, 'codex', 'low').models).toEqual(launch.models);
  // anything malformed reads as nothing, and a write still keeps the rest
  expect(defaultsOf('nope', 'claude')).toEqual({ model: '', effort: '' });
  expect(withModel({ models: { codex: 'x' }, other: 1 }, 'claude', 'opus', 'Opus')).toMatchObject({ models: { codex: 'x', claude: 'opus' }, other: 1 });
});

test('a_saved_model_the_harness_no_longer_lists_warns_and_resets_settings_audit_r1', () => {
  const models = [{ id: 'opus' }, { id: 'sonnet' }];
  expect(staleModel('opus-3', models)).toBe(true);
  expect(staleModel('opus', models)).toBe(false);
  expect(staleModel('', models), 'its own default').toBe(false);
  expect(staleModel('opus-3', []), 'nothing listed yet: unknown').toBe(false);
  // Reset: back to the harness's own default, the others' kept
  const reset = withModel({ models: { claude: 'opus-3', codex: 'gpt-5' }, labels: { claude: 'Opus 3' } }, 'claude', '', '');
  expect(reset.models).toEqual({ codex: 'gpt-5' });
  expect(reset.labels).toEqual({});
  expect(staleModel(defaultsOf(reset, 'claude').model, models)).toBe(false);
  // the detail shows the warning with its Reset
  expect(DETAIL).toContain('part="accounts.defaults.stale"');
});

test('the_list_walks_with_up_down_home_end_and_enter_goes_into_the_detail_settings_audit_r1', () => {
  const ids = ['all', 'claude', 'codex', 'grok'];
  expect(listStep('ArrowDown', ids, 'claude')).toEqual({ pick: 'codex' });
  expect(listStep('ArrowUp', ids, 'claude')).toEqual({ pick: 'all' });
  expect(listStep('ArrowDown', ids, 'grok'), 'the end stays').toEqual({ pick: 'grok' });
  expect(listStep('ArrowUp', ids, 'all'), 'the top stays').toEqual({ pick: 'all' });
  expect(listStep('Home', ids, 'codex')).toEqual({ pick: 'all' });
  expect(listStep('End', ids, 'claude')).toEqual({ pick: 'grok' });
  expect(listStep('Enter', ids, 'codex')).toEqual({ detail: true });
  expect(listStep('ArrowRight', ids, 'codex')).toEqual({ detail: true });
  expect(listStep('a', ids, 'codex')).toBeUndefined();
  expect(listStep('ArrowDown', [], 'x')).toBeUndefined();
  // the pick is in the Settings params: a reload, Back and the search land on it
  expect(harnessFilter('codex')).toBe('harness:codex');
  expect(pickOf('harness:codex', ids)).toBe('codex');
  expect(pickOf('harness:gone', ids)).toBe('all');
  expect(pickOf(undefined, ids)).toBe('all');
});

test('a_harnesss_own_ask_near_the_limit_and_its_ring_settings_audit_r1', () => {
  expect(holdOf({ codex: '80' }, 'codex')).toBe('80');
  expect(holdOf({ codex: '80' }, 'claude')).toBe('same');
  expect(withHold({ codex: '80', grok: 'off' }, 'codex', 'same')).toEqual({ grok: 'off' });
  expect(withHold({ grok: 'off' }, 'claude', '95')).toEqual({ grok: 'off', claude: '95' });
  expect(fullest([{ windows: [{ id: 'five_hour', label: '5-hour', used: 0.4 }] }, { windows: [{ id: 'seven_day', label: 'Week', used: 0.7 }] }])?.id).toBe('seven_day');
  expect(fullest([]), 'no plan reported: no ring').toBeUndefined();
});

test('accounts_update_check_has_a_row_settings_search_lands_on_settings_audit_r1', () => {
  expect(PANE_KEYS.agents).toContain('accounts.updateCheck');
  expect(DETAIL).toContain('settingKey="accounts.updateCheck"');
  expect(DETAIL).toContain('settingKey="runtime.leanRuns"');
});
