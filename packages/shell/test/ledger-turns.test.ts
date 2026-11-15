// A turn row as the history ledger reads it (ADR-0068, src/frame/ledger-turns.ts): led by its hero, its outcome from
// its status and hero, wisp's notices told apart from the user's prompts, and the paths and Shipped read the same from
// a whole turn and from the window's stub of it.
import { describe, expect, it } from 'vitest';
import { clause, isNotice, ledgerTurn, outcomeOf, shippedOf, type TurnLike } from '@wisp/shell/pure';

const row = (o: Partial<TurnLike> = {}, body: TurnLike['body'] = {}): TurnLike => ({ id: 't1', idx: 4, prompt: 'fix it\nplease', started_at: 1000, status: 'done', body, ...o });
const kind = (by: string): string => `<${by}>`;

describe('ledger turns', () => {
  it('a_row_is_led_by_its_hero_numbered_from_one_with_its_prompt_whole', () => {
    const t = ledgerTurn(row({}, { presentation: { blocks: [{ type: 'hero', title: 'Reconnect test fixed', status: 'done' }] }, ext: { 'wisp.runtime': { changes: [{ file: 'a.ts' }, { file: 'a.ts' }] } } }), kind);
    expect(t).toEqual({ id: 't1', n: 5, at: 1000, hero: 'Reconnect test fixed', prompt: 'fix it\nplease', outcome: 'done', files: ['a.ts'], shipped: false });
  });

  it('wisp_notices_are_told_apart_from_the_users_prompts_and_a_workflow_step_is_the_users', () => {
    expect([undefined, '', 'user', 'you', 'step'].map(isNotice)).toEqual([false, false, false, false, false]);
    expect(['wisp', 'later', 'crew', 'lead', 'compact', 'channel'].every(isNotice)).toBe(true);
    expect(ledgerTurn(row({ prompt: 'CI finished on the open PR: 7 of 7 jobs passed.' }, { by: 'wisp' }), kind).notice).toBe('CI finished on the open PR');
    expect(ledgerTurn(row({}, { by: 'later' }), kind).notice).toBe('<later>');
    expect(ledgerTurn(row({ prompt: '' }, { by: 'wisp' }), kind).notice).toBe('<wisp>');
    expect('notice' in ledgerTurn(row({}, { by: 'step' }), kind)).toBe(false);
  });

  it('a_notices_kind_is_its_first_clause_cut_at_48', () => {
    expect(clause('Subagents you delegated to earlier have finished: coder-r41.')).toBe('Subagents you delegated to earlier have finished');
    expect(clause('wisp restarted and resumed this task.')).toBe('wisp restarted and resumed this task');
    expect(clause('see v1.2 notes')).toBe('see v1.2 notes');
    expect(clause('x'.repeat(80))).toBe(`${'x'.repeat(47)}…`);
  });

  it('the_outcome_is_failed_on_an_error_a_refusal_or_a_blocked_hero_done_when_it_changed_files_else_an_answer', () => {
    expect(outcomeOf('error', {}, 0)).toBe('failed');
    expect(outcomeOf('done', { isError: true }, 0)).toBe('failed');
    expect(outcomeOf('done', { refusal: { line: 'no' } }, 0)).toBe('failed');
    expect(outcomeOf('done', { presentation: { blocks: [{ type: 'hero', status: 'blocked' }] } }, 3)).toBe('failed');
    expect(outcomeOf('done', { presentation: { blocks: [{ type: 'hero', status: 'info' }] } }, 3)).toBe('answer');
    expect(outcomeOf('done', { presentation: { blocks: [{ type: 'hero', status: 'done' }] } }, 2)).toBe('done');
    expect(outcomeOf('done', { changedFiles: 1 }, 0)).toBe('done');
    expect(outcomeOf('done', {}, 0)).toBe('answer');
    expect(outcomeOf('running', {}, 0)).toBe('running');
    expect(outcomeOf('held', {}, 0)).toBe('running');
  });

  it('a_stub_reads_the_same_as_its_whole_turn_paths_and_shipped_from_its_own_fields', () => {
    const blocks = [{ type: 'hero', title: 'PR #795 opened', status: 'done' }, { type: 'graph', state: 'open' }];
    const whole = ledgerTurn(row({}, { presentation: { blocks }, ext: { 'wisp.runtime': { changes: [{ file: 'x.ts' }] } } }), kind);
    const stub = ledgerTurn(row({}, { presentation: { blocks: [blocks[0] as (typeof blocks)[number]] }, files: ['x.ts'], shipped: true, changedFiles: 1 }), kind);
    expect(stub).toEqual(whole);
    expect(whole.shipped).toBe(true);
  });

  it('shipped_is_a_commits_block_with_commits_or_an_open_or_merged_pr', () => {
    expect(shippedOf({ presentation: { blocks: [{ type: 'commits', items: [{}] }] } })).toBe(true);
    expect(shippedOf({ presentation: { blocks: [{ type: 'commits', items: [] }] } })).toBe(false);
    expect(shippedOf({ presentation: { blocks: [{ type: 'graph', state: 'merged' }] } })).toBe(true);
    expect(shippedOf({ presentation: { blocks: [{ type: 'graph', state: 'closed' }] } })).toBe(false);
    expect(shippedOf({ changes: [{ file: 'v1.ts' }] })).toBe(false);
  });
});
