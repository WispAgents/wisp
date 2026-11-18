// docs/ux/2026-11-18-desktop.md #2: a task row is two lines (title and time; the harness's logo and the answer's last
// line, else harness · model), the machine only when the task isn't on this computer, and the time's slot swaps to
// Later, Done (Reopen when done) and ⋯ on hover and keyboard focus.
import { expect, test } from 'vitest';
import { rowView, secondLine, type RowContext } from '../src/model/row.ts';
import type { SessionRow } from '../src/types.ts';

const row = (o: Partial<SessionRow>): SessionRow => ({ id: 's1', title: 'Fix login', agent: 'claude', state: 'review', createdAt: 0, updatedAt: 0, ...o }) as SessionRow;
const c = (o: Partial<RowContext> = {}): RowContext => ({ split: true, open: false, swipes: false, here: 'This computer', agents: new Map() as unknown as RowContext['agents'], ...o });

test('done_rows_offer_reopen_in_the_done_slot_review_rows_mark_done_ux_2', () => {
  expect(rowView(row({ state: 'review' }), c()).quickDone).toBe('show');
  expect(rowView(row({ state: 'done' }), c()).quickDone).toBe('reopen');
  // archived, a subagent or the phone app's swipes: none
  expect(rowView(row({ state: 'done', archived: true } as Partial<SessionRow>), c()).quickDone).toBe('gap');
  expect(rowView(row({ state: 'done' }), c({ swipes: true })).quickDone).toBe('none');
});

test('the_second_line_is_the_answer_else_the_run_and_the_machine_only_when_remote_ux_2', () => {
  const answered = rowView(row({ preview: 'Added a limiter to /login' } as Partial<SessionRow>), c());
  expect(secondLine(answered)).toEqual({ line: 'sub', where: false });
  const remote = rowView(row({ preview: 'Running tests…', machine: { id: 'm1', name: 'gpu-box' } } as Partial<SessionRow>), c());
  expect(secondLine(remote)).toEqual({ line: 'sub', where: true });
  const bare = rowView(row({ state: 'done' }), c());
  expect(secondLine(bare).line).toBe(bare.sub ? 'sub' : 'run');
  expect(secondLine(bare).where).toBe(false);
});
