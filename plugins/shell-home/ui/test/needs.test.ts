import { describe, expect, it } from 'vitest';
import { asking, needsList, needsYou, stateKey } from '../src/needs.ts';
import type { TaskRow } from '../src/types.ts';

const row = (id: string, updatedAt: number, more: Partial<TaskRow> = {}): TaskRow => ({ id, title: id, state: 'review', updatedAt, ...more });

describe('needs you on Home', () => {
  it('a_task_asking_you_comes_first_then_newest_as_the_sidebars_needs_you_v1_feed_groups', () => {
    const fillers = [1, 2, 3, 4, 5, 6].map((n) => row(`filler-${n}`, 100 + n));
    const asking = row('ask', 10, { question: 'Which branch?' });
    const approving = row('approve', 5, { approval: true });
    const ids = needsList([...fillers, approving, asking]).map((r) => r.id);
    expect(ids.slice(0, 2)).toEqual(['ask', 'approve']);
    expect(ids.slice(2, 5)).toEqual(['filler-6', 'filler-5', 'filler-4']);
  });
});

describe('a runtime.ask', () => {
  it('a_task_waiting_with_an_empty_question_counts_as_asking_audit_d_1', () => {
    const ask = row('ask1', 1, { state: 'waiting', question: '' });
    expect([asking(ask), needsYou(ask), stateKey(ask)]).toEqual([true, true, 'shell-home.state.asking']);
    expect(needsList([row('f', 9), ask]).map((r) => r.id)).toEqual(['ask1', 'f']);
  });
});

describe('a run you stopped', () => {
  it('says_stopped_not_blocked_audit_G15', () => {
    expect(stateKey(row('run', 1, { status: 'stopped' }))).toBe('shell-home.state.stopped');
    expect(stateKey(row('run', 1, { status: 'blocked' }))).toBe('shell-home.state.blocked');
  });
});
