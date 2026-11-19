import { describe, expect, it } from 'vitest';
import { needsList } from '../src/needs.ts';
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
