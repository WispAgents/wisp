// Plan budget's words past the bars (plan.ts; v1 features/budget.js windowLine, bankedHtml, whereHtml, compareHtml).
import { expect, test } from 'vitest';
import {
  bankedSoon,
  bankedWords,
  compareParts,
  compareWords,
  HARNESSES,
  harnessesOf,
  harnessUsageDescription,
  limitsReported,
  overWords,
  plannedHarnesses,
  stopKeyOf,
  stopLine,
  tokenBreakdown,
  whereRows,
  windowLine,
  type Compare,
} from '../src/plan.ts';
import { cacheHit, costCell, costWords, limitLeft, stackedModel, usageModel, weekStart } from '../src/usage-model.ts';

const when = (at: number): string => `@${at}`;
const H = 3_600_000;

test('plan_budget_shows_each_accounts_windows_with_banked_codex_resets', () => {
  // the line under each bar: one thing each, no time twice
  expect(windowLine({ used: 0.2 }, null, when)).toBe('New window');
  expect(windowLine({ used: 1, resetsAt: 5 }, null, when)).toBe('Used up · resets @5');
  expect(windowLine({ used: 0.8, resetsAt: 5 }, 0.75, when)).toBe('Automations wait · resets @5');
  expect(windowLine({ used: 0.1, resetsAt: 5, earlyAt: 2 }, null, when)).toBe('Reset early @2 · resets @5');
  expect(windowLine({ used: 0.5, resetsAt: 5, hitAt: 4 }, null, when)).toBe('Resets @5 · at this pace, runs out @4');
  expect(windowLine({ used: 0.5, resetsAt: 5 }, null, when)).toBe('Resets @5');
  // a harness's own stop line wins over Automations stop at; 'same' follows it; off: never
  expect(stopLine('75', '50')).toBe(0.75);
  expect(stopLine('same', '50')).toBe(0.5);
  expect(stopLine(undefined, 'off')).toBeNull();
  // banked resets: the count, and the next to expire
  const now = 1_000 * H;
  expect(bankedWords(null, now, when)).toBe('');
  expect(bankedWords({ count: 2, credits: null }, now, when)).toBe('2 banked resets');
  expect(bankedWords({ count: 1, credits: [{ id: 'a', expiresAt: now + 2 * H }] }, now, when)).toBe(`1 banked reset, expires @${now + 2 * H}`);
  expect(bankedWords({ count: 2, credits: [{ id: 'a', expiresAt: now - 1 }, { id: 'b', expiresAt: now + 50 * H }] }, now, when)).toBe(`2 banked resets, the next expires @${now + 50 * H}`);
  expect(bankedSoon({ count: 1, credits: [{ id: 'a', expiresAt: now + 2 * H }] }, now)).toBe(true);
  expect(bankedSoon({ count: 1, credits: [{ id: 'a', expiresAt: now + 50 * H }] }, now)).toBe(false);
  expect(overWords({ runs: 2, at: 9, kind: 'credits' }, when)).toBe('2 runs in the last 7 days billed to Codex credits (last: @9).');
});

test('where_your_plan_went_lists_the_top_tasks_the_rest_and_what_ran_outside_wisp', () => {
  expect(whereRows(null)).toBeUndefined();
  const w = whereRows({ window: 'seven_day', label: 'Weekly', used: 0.5, traced: 0.4, tasks: [{ id: 's1', title: 'Fix login', used: 0.25 }, { id: 's2', title: 'Docs', used: 0.1 }], more: 3 });
  expect(w?.heading).toBe('Where the weekly window went');
  expect(w?.rows.map((r) => [r.title, r.pct, r.id ?? ''])).toEqual([
    ['Fix login', '25%', 's1'],
    ['Docs', '10%', 's2'],
    ['3 other tasks', '5%', ''],
    ['Outside wisp or earlier', '10%', ''],
  ]);
});

test('two_accounts_show_tokens_per_model_per_account', () => {
  const c: Compare = {
    model: 'opus',
    accounts: [
      { account: 'w1', turns: 9, output: 19_000, input: 40_000, cache: 0.71 },
      { account: 'default', turns: 12, output: 10_000, input: 38_000, cache: null },
    ],
  };
  const label = (a: string): string => (a === 'w1' ? 'Work' : 'Personal');
  expect(compareWords(c, label)).toBe('Work uses 1.9× the output tokens per turn of Personal (n=9 / 12)');
  expect(compareParts(c.accounts[0] as Compare['accounts'][number])).toEqual(['19k out', '40k in', '71% cache', '9 turns']);
  expect(compareParts(c.accounts[1] as Compare['accounts'][number])).toEqual(['10k out', '38k in', '12 turns']);
  const same: Compare = { model: '', accounts: c.accounts.map((x) => ({ ...x, output: 10_000 })) };
  expect(compareWords(same, label)).toBe('Work and Personal use about the same output tokens per turn (n=9 / 12)');
});

test('every_harness_has_a_stop_key_and_per_harness_usage_summary_and_chart_metrics_try_32', () => {
  expect(HARNESSES.map((h) => [h.agent, stopKeyOf(h.agent)])).toEqual([
    ['claude', 'autoStopClaude'],
    ['codex', 'autoStopCodex'],
    ['antigravity', 'autoStopAntigravity'],
    ['opencode', 'autoStopOpencode'],
    ['cursor', 'autoStopCursor'],
    ['copilot', 'autoStopCopilot'],
    ['grok', 'autoStopGrok'],
  ]);
  expect(plannedHarnesses().map((h) => h.agent)).toEqual(['claude', 'codex', 'antigravity']);
  // budget_acc ids as store.rs writes them: `{agent}|{account}`, and a peer's `peer:{agent}|{account}`
  const base = ['claude', 'codex', 'antigravity'];
  expect(plannedHarnesses([{ id: 'cursor|default' }]).map((h) => h.agent)).toEqual([...base, 'cursor']);
  expect(plannedHarnesses([{ id: 'local:cursor|default' }]).map((h) => h.agent)).toEqual([...base, 'cursor']);
  expect(plannedHarnesses([{ id: 'copilot|work' }, { id: 'local:grok|default' }]).map((h) => h.agent)).toEqual([...base, 'copilot', 'grok']);
  expect(plannedHarnesses([{ id: 'nobody|default' }]).map((h) => h.agent)).toEqual(base);
  expect(plannedHarnesses([{ id: 'cursor|default', body: { agent: 'grok' } }]).map((h) => h.agent)).toEqual([...base, 'grok']);

  const u = {
    from: 0,
    spend: 0.09,
    turns: 3,
    worked: 100,
    questions: 0,
    priced: 1,
    sessions: 2,
    daily: [
      { date: '2026-11-11', cost: 0, turns: 1, tokens: 44_000 },
      { date: '2026-11-12', cost: 0.09, turns: 2, tokens: 112_000 },
    ],
    runners: [
      { key: 'claude opus', name: 'opus', agent: 'claude', cost: 0.09, priced: 1, turns: 1, sessions: 1, tokens: 80_000, inputTokens: 60_000, outputTokens: 20_000, cacheTokens: 30_000, worked: 100, questions: 0, avgMs: 20_000 },
      { key: 'claude sonnet', name: 'sonnet', agent: 'claude', cost: 0, priced: 0, turns: 1, sessions: 1, tokens: 32_000, inputTokens: 24_000, outputTokens: 8_000, cacheTokens: 0, worked: 100, questions: 0, avgMs: 10_000 },
      { key: 'codex gpt-5.5', name: 'gpt-5.5', agent: 'codex', cost: 0, priced: 0, turns: 1, sessions: 1, tokens: 44_000, inputTokens: 36_000, outputTokens: 8_000, cacheTokens: 18_000, worked: 100, questions: 0, avgMs: 30_000 },
    ],
  };
  const rolled = harnessesOf(u);
  expect(rolled.map((r) => [r.agent, r.turns, r.tokens, r.cost])).toEqual([
    ['claude', 2, 112_000, 0.09],
    ['codex', 1, 44_000, 0],
  ]);
  expect(tokenBreakdown(rolled[0] ?? {})).toBe('84k in · 36% cache · 28k out');

  const claudeMeta = HARNESSES[0]!;
  expect(
    harnessUsageDescription(claudeMeta, rolled[0], [{ account: 'default', windows: [{ id: 'seven_day', label: 'Weekly', used: 0.64 }] }]),
  ).toContain('2 turns · 112k tokens · $0.09+ · 100% worked · Weekly 64%'); // one of the two turns priced

  const byTokens = usageModel(u, 'tokens');
  expect(byTokens.active).toBe('tokens');
  expect(byTokens.max).toBe('max 112k/day');
  expect(byTokens.bars[1]?.tip).toContain('112k tokens');
});

test('a_harness_that_reports_no_windows_says_so_and_a_tokens_only_range_is_labelled_tokens', () => {
  const cursor = HARNESSES.find((h) => h.agent === 'cursor')!;
  const grok = HARNESSES.find((h) => h.agent === 'grok')!;
  expect(limitsReported(cursor, [])).toBe(false);
  expect(limitsReported(cursor, [{ account: 'default', windows: [{ id: 'w', label: 'Monthly', used: 0.2 }] }])).toBe(true);
  expect(limitsReported(HARNESSES[0]!, []), 'Claude reports windows by design').toBe(true);
  expect(harnessUsageDescription(cursor, undefined, [])).toBe('Tracks tokens, turns and duration · limits not reported by Cursor');
  expect(harnessUsageDescription(grok, undefined, [])).toBe('Tracks tokens, spend and outcomes · limits not reported by Grok');
  expect(limitLeft(cursor, [], 0)).toEqual({ none: 'limits not reported by Cursor' });
  const now = 1_000 * H;
  const claude = [{ account: 'a', windows: [{ id: 'five_hour', label: 'Session', used: 0.3 }, { id: 'seven_day', label: 'Weekly', used: 0.64, resetsAt: now + 2 * H + 10 * 60_000 }] }];
  expect(limitLeft(HARNESSES[0]!, claude, now)).toEqual({ left: '36% of weekly left', resets: 'resets in 2h 10m', full: false });
  // nothing priced: the first tile counts tokens, so it never says Spend
  const tokensOnly = { from: 0, spend: 0, turns: 2, worked: 100, questions: 0, priced: 0, tokens: 9_000, daily: [{ date: '2026-11-12', cost: 0, turns: 2, tokens: 9_000 }] };
  expect(usageModel(tokensOnly).spendLabel).toBe('Tokens');
  expect(usageModel({ ...tokensOnly, priced: 1, spend: 0.1 }).spendLabel).toBe('Spend');
});

test('the_usage_page_stacks_harnesses_per_day_or_week_and_reads_cache_hit', () => {
  const u = {
    from: 0,
    spend: 0.6,
    turns: 5,
    worked: 100,
    questions: 0,
    priced: 2,
    daily: ['2026-11-22', '2026-11-23', '2026-11-24'].map((date) => ({ date, cost: 0, turns: 0 })),
    dailyByHarness: [
      { date: '2026-11-22', agent: 'codex', cost: 0, tokens: 4_000, cache_read: 0, turns: 1 },
      { date: '2026-11-23', agent: 'claude', cost: 0.4, tokens: 6_000, cache_read: 1_000, turns: 2 },
      { date: '2026-11-23', agent: 'codex', cost: 0, tokens: 2_000, cache_read: 0, turns: 1 },
      { date: '2026-11-24', agent: 'claude', cost: 0.2, tokens: 1_000, cache_read: 0, turns: 1 },
    ],
  };
  const m = stackedModel(u, 'tokens');
  expect(m.series.map((s) => [s.key, s.series])).toEqual([
    ['claude', 0],
    ['codex', 1],
  ]);
  expect(m.bars.map((b) => [b.key, b.share, b.stack.map((x) => [x.series, x.share])])).toEqual([
    ['2026-11-22', 0.5, [[1, 0.5]]],
    ['2026-11-23', 1, [[0, 0.75], [1, 0.25]]],
    ['2026-11-24', 0.125, [[0, 0.125]]],
  ]);
  expect(m.bars[1]?.tip).toBe('Nov 23: Claude Code 6k · Codex 2k');
  expect(m.max).toBe('max 8k/day');
  // hiding Codex keeps Claude's colour and rescales the bars to what's left
  const hid = stackedModel(u, 'tokens', { hidden: new Set(['codex']) });
  expect(hid.series.map((s) => [s.key, s.series, s.off])).toEqual([
    ['claude', 0, false],
    ['codex', 1, true],
  ]);
  expect(hid.bars.map((b) => b.share)).toEqual([0, 1, 1 / 6]);
  // by week: Sunday Nov 22 is in the week of Monday Nov 16, Nov 23–24 in the next
  expect(weekStart('2026-11-22')).toBe('2026-11-16');
  expect(weekStart('2026-11-23')).toBe('2026-11-23');
  expect(weekStart('2027-01-01')).toBe('2026-12-28');
  const wk = stackedModel(u, 'turns', { scale: 'week' });
  expect(wk.bars.map((b) => [b.key, b.stack.map((x) => x.share)])).toEqual([
    ['2026-11-16', [0.25]],
    ['2026-11-23', [0.75, 0.25]],
  ]);
  expect(wk.first).toBe('Week of Nov 16');
  expect(wk.max).toBe('max 4/week');
  // cache hit: no input reads '—', never NaN
  expect(cacheHit({ inputTokens: 0, cacheTokens: 0 })).toBe('—');
  expect(cacheHit({})).toBe('—');
  expect(cacheHit({ inputTokens: 40_000, cacheTokens: 30_000 })).toBe('75%');
});

test('an_estimated_cost_reads_tilde_and_a_partly_priced_row_is_marked', () => {
  expect(costWords(1.234)).toBe('$1.23');
  expect(costWords(1.234, { estimated: true })).toBe('~$1.23');
  expect(costWords(2, { estimated: true, priced: 3, turns: 5 })).toBe('~$2.00+');
  expect(costWords(2, { priced: 5, turns: 5 })).toBe('$2.00');
  const row = { cost: 2, priced: 3, turns: 5, costEstimated: true, estimated: 2 };
  expect(costCell(row)).toEqual({ text: '~$2.00+', tip: '2 turns estimated from their tokens at API list price. 3 of 5 turns priced.' });
  expect(costCell({ cost: 0, priced: 0, turns: 4 })).toEqual({ text: '—', tip: '' });
  const u = { from: 0, spend: 1.5, turns: 4, worked: 100, questions: 0, priced: 3, costEstimated: true, daily: [{ date: '2026-11-23', cost: 1.5, turns: 4, cost_estimated: true }] };
  const m = usageModel(u);
  expect(m.spend).toBe('~$1.50+');
  expect(m.bars[0]?.tip).toBe('Nov 23: ~$1.50 · 4 turns');
  expect(harnessUsageDescription(HARNESSES[1]!, { key: 'codex', name: 'codex', agent: 'codex', cost: 1.5, priced: 3, turns: 4, costEstimated: true, worked: null, questions: 0, avgMs: null })).toContain('~$1.50+');
});

test('the_week_scale_reads_the_runtimes_weekly_rollup', () => {
  const u = {
    from: 0,
    spend: 0,
    turns: 6,
    worked: 100,
    questions: 0,
    priced: 0,
    tokens: 10,
    daily: [{ date: '2026-11-22', cost: 0, turns: 2, tokens: 4 }, { date: '2026-11-23', cost: 0, turns: 4, tokens: 6 }],
    weekly: [
      { week: '2026-W47', start: '2026-11-16', cost: 0, turns: 2, tokens: 4 },
      { week: '2026-W48', start: '2026-11-23', cost: 0, turns: 4, tokens: 6 },
    ],
  };
  const w = usageModel(u, 'turns', 'week');
  expect(w.bars.map((b) => [b.key, b.share])).toEqual([
    ['2026-11-16', 0.5],
    ['2026-11-23', 1],
  ]);
  expect(w.first).toBe('Week of Nov 16');
  expect(w.max).toBe('max 4/week');
  expect(usageModel(u, 'turns').bars.length).toBe(2);
});
