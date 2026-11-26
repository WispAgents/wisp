// Audit user-side-edits-for-approval: a side discussion's write to a file the main task changed waits on the main task
// as a card: the change as a diff, which side discussion it came from, Apply and Discard. And the commands its buttons
// send take the card off at once (a stale Apply's Conflict brings it back with a toast).
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import Proposals from '../src/frame/Proposals.tsx';
import { proposalApply, proposalsOf, type ProposalRow } from '../src/frame/commands.ts';
import type { SessionRow } from '../src/rows.ts';

const off: (() => void)[] = [];
afterEach(() => {
  while (off.length) off.pop()?.();
  document.body.innerHTML = '';
});

const p: ProposalRow = { id: 'p1', side: 's1', sideTitle: 'Why the retry?', path: 'src/a.ts', before: 'one\ntwo\n', after: 'one\nTWO\n', at: 1 };
const t = (key: string, vars?: Record<string, string>): string => `${key}${vars ? ` ${Object.values(vars).join(' ')}` : ''}`;

test('a_proposal_shows_its_diff_and_where_it_came_from_and_its_buttons_apply_or_discard_it', () => {
  const host = document.createElement('div');
  document.body.append(host);
  const applied: string[] = [];
  const discarded: string[] = [];
  const opened: string[] = [];
  off.push(render(() => <Proposals items={[p]} t={t} onApply={(id) => applied.push(id)} onDiscard={(id) => discarded.push(id)} onOpenSide={(s) => opened.push(s)} />, host));
  const card = host.querySelector('[data-part="task.proposal"]');
  expect(card?.textContent).toContain('shell.proposal.head src/a.ts');
  const diff = host.querySelector('[data-part="task.proposal.diff"]')?.textContent ?? '';
  expect(diff).toContain('two');
  expect(diff).toContain('TWO');
  (host.querySelector('[data-part="task.proposal.apply"]') as HTMLButtonElement).click();
  (host.querySelector('[data-part="task.proposal.discard"]') as HTMLButtonElement).click();
  (host.querySelector('[data-part="task.proposal.side"]') as HTMLButtonElement).click();
  expect([applied, discarded, opened]).toEqual([['p1'], ['p1'], ['s1']]);
});

test('no_proposal_no_card', () => {
  const host = document.createElement('div');
  document.body.append(host);
  off.push(render(() => <Proposals items={[]} t={t} onApply={() => undefined} onDiscard={() => undefined} />, host));
  expect(host.querySelector('[data-part="task.proposals"]')).toBeNull();
});

test('the_task_row_carries_its_proposals_and_apply_takes_the_card_off_at_once', () => {
  const row = { id: 'local:m1', body: { ext: { 'wisp.runtime': { proposals: [p, { ...p, id: 'p2' }], btw: [] } } } } as unknown as SessionRow;
  expect(proposalsOf(row).map((x) => x.id)).toEqual(['p1', 'p2']);
  expect(proposalsOf(undefined)).toEqual([]);
  expect(proposalApply.wire?.({ id: 'local:m1', proposal: 'p1' })).toEqual({ id: 'm1', proposal: 'p1' });
  const base = { peek: () => row } as unknown as Parameters<Exclude<typeof proposalApply.optimistic, 'none'>>[0];
  const optimistic = proposalApply.optimistic;
  if (optimistic === 'none') throw new Error('optimistic');
  const ops = optimistic(base, { id: 'local:m1', proposal: 'p1' });
  const value = (ops[0] as { ops: { value: { 'wisp.runtime': { proposals: ProposalRow[]; btw: unknown[] } } }[] }).ops[0]?.value;
  expect(value?.['wisp.runtime'].proposals.map((x) => x.id)).toEqual(['p2']);
  expect(value?.['wisp.runtime'].btw, 'the rest of its slot kept').toEqual([]);
});
