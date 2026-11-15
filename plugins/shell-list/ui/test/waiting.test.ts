// A command made offline (ADR-0065 §4): the row shows its result at once and says *Waiting to send* in the status
// column, where the state pill sits, so nothing else in the row moves.
import { expect, test } from 'vitest';
import { pillsOf, rowView, type RowContext } from '../src/model/row.ts';
import type { SessionRow } from '../src/types.ts';
import en from '../src/i18n/en.json' with { type: 'json' };

const row = (o: Partial<SessionRow>): SessionRow => ({ id: 's1', title: 'Fix login', agent: 'claude', state: 'review', status: 'done', createdAt: 0, updatedAt: 0, ...o }) as SessionRow;
const c = (o: Partial<RowContext> = {}): RowContext => ({ split: true, open: false, swipes: false, here: 'This computer', agents: new Map() as unknown as RowContext['agents'], ...o });

test('a_waiting_row_says_waiting_to_send_in_its_status_column', () => {
  const plain = rowView(row({ pinned: true } as Partial<SessionRow>), c());
  const waiting = rowView(row({ pinned: true } as Partial<SessionRow>), c({ waiting: true }));
  expect(waiting.pills[0]?.id).toBe('status');
  expect(waiting.pills[0]?.label.key).toBe('shell-list.pill.waiting');
  expect((en as Record<string, string>)['shell-list.pill.waiting']).toBe('Waiting to send');
  // the same pills after it, in the same places
  expect(waiting.pills.map((p) => p.id)).toEqual(plain.pills.map((p) => p.id));
  // a row with no state pill (working) still gets one: the change on screen isn't sent yet
  expect(pillsOf(row({ state: 'running' }), true)[0]?.label.key).toBe('shell-list.pill.waiting');
  expect(pillsOf(row({}), false)[0]?.label.key).toBe('shell-list.status.done');
});
