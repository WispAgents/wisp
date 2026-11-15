// The task history's ledger, as plain data (ADR-0068, src/app/ledger-model.ts): today and yesterday open, earlier
// days fold into a card, runs of 3+ wisp notices fold into one line, search and filters give a flat list by day, the
// scrubber has one tick per turn, and j/k and [ / ] step through turns and failures.
import { describe, expect, it } from 'vitest';
import {
  FOLD_RUN,
  counts,
  dayKind,
  dayOf,
  dayOpen,
  dayStats,
  daysOf,
  foldNotices,
  ledgerKey,
  ledgerRows,
  matches,
  nextFailure,
  stepFrom,
  ticks,
  titleOf,
  turnAt,
  type LedgerTurn,
  type LedgerView,
} from '@wisp/ui/pure';

const H = 3_600_000;
// noon today, local time
const NOW = ((d) => (d.setHours(12, 0, 0, 0), d.getTime()))(new Date(2026, 10, 15));
const turn = (n: number, at: number, o: Partial<LedgerTurn> = {}): LedgerTurn => ({
  id: `t${n}`,
  n,
  at,
  hero: `Hero ${n}`,
  prompt: `ask ${n}\nmore`,
  outcome: 'done',
  files: [],
  shipped: false,
  ...o,
});
const view = (o: Partial<LedgerView> = {}): LedgerView => ({ q: '', filter: 'all', days: new Map(), unfolded: new Set(), open: undefined, ...o });

describe('days', () => {
  it('today_and_yesterday_open_earlier_days_fold_into_a_card_with_counts_span_files_and_a_strip', () => {
    const ts = [
      turn(1, NOW - 3 * 24 * H, { outcome: 'failed', files: ['a.ts'] }),
      turn(2, NOW - 3 * 24 * H + H, { files: ['a.ts', 'b.ts'] }),
      turn(3, NOW - 3 * 24 * H + 2 * H, { notice: 'CI finished' }),
      turn(4, NOW - 24 * H),
      turn(5, NOW - H),
    ];
    const rows = ledgerRows(ts, NOW, view());
    expect(rows.map((r) => r.k)).toEqual(['card', 'day', 'turn', 'day', 'turn']);
    const card = rows[0];
    if (card?.k !== 'card') throw new Error('card');
    expect(card.stats).toEqual({ count: 3, from: ts[0]?.at, to: ts[2]?.at, done: 2, failed: 1, files: 2 });
    expect(card.strip).toEqual(['failed', 'done', 'notice']);
    expect(rows[1]).toMatchObject({ k: 'day', kind: 'yesterday', count: 1 });
    expect(rows[3]).toMatchObject({ k: 'day', kind: 'today', count: 1 });
  });

  it('a_day_opens_or_folds_by_hand_and_the_open_turns_day_always_shows', () => {
    const ts = [turn(1, NOW - 4 * 24 * H), turn(2, NOW - H)];
    expect(ledgerRows(ts, NOW, view({ days: new Map([[dayOf(ts[0]?.at ?? 0), true]]) }))[0]?.k).toBe('day');
    expect(ledgerRows(ts, NOW, view({ days: new Map([[dayOf(NOW), false]]) })).map((r) => r.k)).toEqual(['card', 'card']);
    const d = daysOf(ts, NOW)[0];
    if (!d) throw new Error('day');
    expect(dayOpen(d, { days: new Map([[d.day, false]]), open: 't1' })).toBe(true);
  });

  it('day_kinds_follow_local_midnight', () => {
    expect(dayKind(dayOf(NOW), NOW)).toBe('today');
    expect(dayKind(dayOf(NOW - 24 * H), NOW)).toBe('yesterday');
    expect(dayKind(dayOf(NOW - 48 * H), NOW)).toBe('earlier');
    // just after midnight, a turn from 23:50 is yesterday's
    const early = ((d) => (d.setHours(0, 5, 0, 0), d.getTime()))(new Date(2026, 10, 15));
    expect(dayKind(dayOf(early - 15 * 60_000), early)).toBe('yesterday');
  });

  it('a_turn_out_of_time_order_joins_its_own_day_which_stays_one_group_with_one_key', () => {
    // idx order, but the second turn's clock says yesterday (an import, a skewed clock)
    const ts = [turn(1, NOW - 2 * H), turn(2, NOW - 24 * H), turn(3, NOW - H)];
    const ds = daysOf(ts, NOW);
    expect(ds.map((d) => [d.kind, d.turns.map((t) => t.id)])).toEqual([
      ['yesterday', ['t2']],
      ['today', ['t1', 't3']],
    ]);
    const keys = ledgerRows(ts, NOW, view()).map((r) => r.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it('day_stats_of_no_turns_are_zero', () => {
    expect(dayStats([])).toEqual({ count: 0, from: 0, to: 0, done: 0, failed: 0, files: 0 });
  });
});

describe('notices', () => {
  const n = (i: number, kind: string): LedgerTurn => turn(i, NOW - 10 * H + i * 60_000, { notice: kind });
  it('three_or_more_notices_in_a_row_fold_into_one_line_with_their_kinds_once_each', () => {
    const ts = [turn(1, NOW - 10 * H), n(2, 'CI finished'), n(3, 'Resumed'), n(4, 'CI finished'), n(5, 'Resumed'), turn(6, NOW - H)];
    const rows = foldNotices(ts, undefined, new Set());
    expect(rows.map((r) => r.k)).toEqual(['turn', 'fold', 'turn']);
    expect(rows[1]).toMatchObject({ k: 'fold', key: 'fold:t2', kinds: ['CI finished', 'Resumed'] });
    if (rows[1]?.k === 'fold') expect(rows[1].turns.map((t) => t.id)).toEqual(['t2', 't3', 't4', 't5']);
  });

  it('two_notices_stay_rows_an_unfolded_run_and_a_run_holding_the_open_turn_show_every_row', () => {
    expect(FOLD_RUN).toBe(3);
    const two = [n(1, 'A'), n(2, 'B'), turn(3, NOW)];
    expect(foldNotices(two, undefined, new Set()).map((r) => r.k)).toEqual(['turn', 'turn', 'turn']);
    const three = [n(1, 'A'), n(2, 'B'), n(3, 'C')];
    expect(foldNotices(three, undefined, new Set(['t1'])).map((r) => r.k)).toEqual(['turn', 'turn', 'turn']);
    expect(foldNotices(three, 't2', new Set()).map((r) => r.k)).toEqual(['turn', 'turn', 'turn']);
    // a run at the very end folds too
    expect(foldNotices([turn(0, NOW), ...three], undefined, new Set()).map((r) => r.k)).toEqual(['turn', 'fold']);
  });
});

describe('search and filters', () => {
  const ts = [
    turn(1, NOW - 50 * H, { hero: 'Reconnect timer fixed', files: ['server/reconnect.js'] }),
    turn(2, NOW - 49 * H, { hero: 'CI failed', outcome: 'failed', notice: 'CI failed' }),
    turn(3, NOW - 2 * H, { hero: 'PR #795 opened', shipped: true, prompt: 'commit and open a PR' }),
    turn(4, NOW - H, { hero: '', prompt: 'why is the test flaky?\nlook', outcome: 'answer' }),
  ];
  it('search_reads_heroes_prompts_and_file_paths_every_word_anywhere_case_blind', () => {
    expect(matches(ts[0] as LedgerTurn, 'RECONNECT.JS', 'all')).toBe(true);
    expect(matches(ts[0] as LedgerTurn, 'timer server/', 'all')).toBe(true);
    expect(matches(ts[2] as LedgerTurn, 'open a pr', 'all')).toBe(true);
    expect(matches(ts[3] as LedgerTurn, 'flaky', 'all')).toBe(true);
    expect(matches(ts[3] as LedgerTurn, 'flaky nope', 'all')).toBe(false);
  });

  it('filters_yours_failed_shipped_and_notices_and_their_counts', () => {
    const ids = (f: LedgerView['filter']): string[] => ts.filter((t) => matches(t, '', f)).map((t) => t.id);
    expect(ids('yours')).toEqual(['t1', 't3', 't4']);
    expect(ids('failed')).toEqual(['t2']);
    expect(ids('shipped')).toEqual(['t3']);
    expect(ids('notices')).toEqual(['t2']);
    expect(counts(ts)).toEqual({ all: 4, yours: 3, failed: 1, shipped: 1, notices: 1 });
  });

  it('searching_gives_a_flat_list_of_matches_under_a_heading_per_day_without_cards_or_folds', () => {
    const rows = ledgerRows(ts, NOW, view({ q: 'e' }));
    expect(rows.map((r) => r.k)).toEqual(['day', 'turn', 'turn', 'day', 'turn', 'turn']);
    expect(rows[0]).toMatchObject({ k: 'day', search: true, count: 2 });
    expect(ledgerRows(ts, NOW, view({ filter: 'failed' })).map((r) => r.key)).toEqual([`day:${dayOf(ts[1]?.at ?? 0)}`, 'turn:t2']);
    expect(ledgerRows(ts, NOW, view({ q: 'zzz' }))).toEqual([]);
  });

  it('a_row_without_a_hero_is_titled_by_its_prompts_first_line', () => {
    expect(titleOf(ts[3] as LedgerTurn)).toBe('why is the test flaky?');
    expect(titleOf(ts[0] as LedgerTurn)).toBe('Reconnect timer fixed');
  });
});

describe('scrubber', () => {
  const ts = [turn(1, 1), turn(2, 2, { outcome: 'failed' }), turn(3, 3, { notice: 'Resumed' }), turn(4, 4)];
  it('one_tick_per_turn_spread_evenly_red_for_failed_faint_for_notices_accent_for_the_open_one', () => {
    expect(ticks(ts, 't4')).toEqual([
      { id: 't1', tone: 'done', at: 0.125 },
      { id: 't2', tone: 'failed', at: 0.375 },
      { id: 't3', tone: 'notice', at: 0.625 },
      { id: 't4', tone: 'on', at: 0.875 },
    ]);
    // each tick sits in the middle of the slot a point there names
    for (const k of ticks(ts, undefined)) expect(turnAt(ts, k.at * 100, 100)?.id).toBe(k.id);
    expect(ticks([], undefined)).toEqual([]);
  });

  it('a_point_on_the_scrubber_names_the_turn_under_it_clamped_to_both_ends', () => {
    expect(turnAt(ts, 0, 100)?.id).toBe('t1');
    expect(turnAt(ts, 26, 100)?.id).toBe('t2');
    expect(turnAt(ts, 100, 100)?.id).toBe('t4');
    expect(turnAt(ts, -40, 100)?.id).toBe('t1');
    expect(turnAt(ts, 500, 100)?.id).toBe('t4');
    expect(turnAt([], 5, 100)).toBeUndefined();
    expect(turnAt(ts, 5, 0)).toBeUndefined();
  });
});

describe('keys', () => {
  const ts = [turn(1, 1), turn(2, 2, { outcome: 'failed' }), turn(3, 3), turn(4, 4, { outcome: 'failed' }), turn(5, 5)];
  it('j_and_k_step_from_the_open_turn_and_with_none_open_k_takes_the_latest_j_the_first', () => {
    expect(stepFrom(ts, 't3', 1)?.id).toBe('t4');
    expect(stepFrom(ts, 't3', -1)?.id).toBe('t2');
    expect(stepFrom(ts, 't5', 1)).toBeUndefined();
    expect(stepFrom(ts, 't1', -1)).toBeUndefined();
    expect(stepFrom(ts, undefined, -1)?.id).toBe('t5');
    expect(stepFrom(ts, undefined, 1)?.id).toBe('t1');
    expect(stepFrom(ts, 'gone', -1)?.id).toBe('t5');
    expect(stepFrom([], undefined, 1)).toBeUndefined();
  });

  it('brackets_jump_to_the_previous_or_next_failure', () => {
    expect(nextFailure(ts, 't3', 1)?.id).toBe('t4');
    expect(nextFailure(ts, 't3', -1)?.id).toBe('t2');
    expect(nextFailure(ts, 't4', 1)).toBeUndefined();
    expect(nextFailure(ts, undefined, -1)?.id).toBe('t4');
    expect(nextFailure(ts, undefined, 1)?.id).toBe('t2');
  });

  it('the_ledger_keys_are_j_k_slash_brackets_and_escape_without_modifiers', () => {
    expect(['j', 'k', '/', ']', '[', 'Escape', 'x'].map((key) => ledgerKey({ key }))).toEqual(['next', 'prev', 'search', 'nextFail', 'prevFail', 'close', undefined]);
    expect(ledgerKey({ key: 'k', metaKey: true })).toBeUndefined();
    expect(ledgerKey({ key: 'j', ctrlKey: true })).toBeUndefined();
  });
});
