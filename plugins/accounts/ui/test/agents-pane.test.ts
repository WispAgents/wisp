// Settings → Agents' rules (words.ts): a missing harness's one-line install, and what Re-check turned up
// (v1 settings.js paintAgents, setup.js setupCommand, recheckSummary).
import { expect, test } from 'vitest';
import type { DetectRow, HarnessRow } from '../src/types.ts';
import { harnessLine, harnessState, howToGet, mergeHarnesses, recheckSummary } from '../src/words.ts';

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
  const { recheck } = await import('../src/commands.ts');
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
