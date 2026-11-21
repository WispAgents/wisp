// QA round 1 #8, #9, #14: Home's Needs you, the project chips and the project switcher counted their own idea of "needs
// you" (every task not running or done), so a task saved for later, waiting on a follow-up or a subagent's run still
// counted there while the list had moved it; and a worktree run (its folder outside the project) belonged to no
// project, so "All 9" next to "wisp · 1 needs you". They now read the list's rules (shell-list model/sections.ts).
import { expect, test } from 'vitest';
import { asking, homeNeeds, needsYou, projectOfRow, type ProjectView } from '../src/frame/rows-view.ts';
import type { SessionRow } from '../src/rows.ts';

const row = (o: Partial<SessionRow> & { sum?: Record<string, unknown>; ext?: Record<string, unknown> } = {}): SessionRow =>
  ({ id: 's', title: 's', state: 'review', agent: 'claude', archived: false, unread: false, created_at: 0, updated_at: 0, summary: { ...o.sum }, body: { ext: o.ext ?? {} }, ...o }) as SessionRow;

test('needs_you_reads_the_lists_rules', () => {
  expect(needsYou(row())).toBe(true);
  expect(needsYou(row({ state: 'waiting' }))).toBe(true);
  expect(needsYou(row({ state: 'running' }))).toBe(false);
  expect(needsYou(row({ state: 'done' }))).toBe(false);
  expect(needsYou(row({ archived: true }))).toBe(false);
  // saved for later: the server's `{at, by}`
  expect(needsYou(row({ sum: { forLater: { at: 1, by: 'you' } } }))).toBe(false);
  // a subagent's run sits under its lead
  expect(needsYou(row({ parent_id: 'lead' }))).toBe(false);
  // waiting on a follow-up: Scheduled
  expect(needsYou(row({ ext: { 'wisp.later': { later: [{ at: 5 }] } } }))).toBe(false);
  // a reminder that came due does need you
  expect(needsYou(row({ sum: { reminder: true }, ext: { 'wisp.later': { later: [{ at: 5, remind: true }] } } }))).toBe(true);
  // a terminal task with its terminal open is being worked on
  expect(needsYou(row({ sum: { tui: true, term: true } }))).toBe(false);
  // as the list's isForLater: saved, but a reminder came due (or a turn waits to send): still needs you
  expect(needsYou(row({ sum: { forLater: { at: 1, by: 'you' }, reminder: true } }))).toBe(true);
  expect(needsYou(row({ sum: { forLater: { at: 1, by: 'you' }, pending: true } }))).toBe(true);
  // as the list's inTerm: only a finished one; one asking you does need you
  expect(needsYou(row({ state: 'waiting', sum: { tui: true, term: true } }))).toBe(true);
});

test('a_worktree_run_belongs_to_the_project_it_was_started_in', () => {
  const ps: ProjectView[] = [{ id: 'wisp', name: 'wisp', path: '/Users/me/wisp', color: 'slate' } as ProjectView];
  // its worktree's repo (wisp.worktrees' root), or the repo's name in the summary
  expect(projectOfRow(row({ sum: { cwd: '/Users/me/.wisp/worktrees/wisp-01a' }, ext: { 'wisp.worktrees': { root: '/Users/me/wisp', path: '/Users/me/.wisp/worktrees/wisp-01a' } } }), ps)?.id).toBe('wisp');
  expect(projectOfRow(row({ sum: { project: 'wisp', cwd: '/Users/me/.wisp/worktrees/wisp-01a' } }), ps)?.id).toBe('wisp');
  expect(projectOfRow(row({ sum: { project: null, cwd: '/Users/me/wisp/src' } }), ps)?.id).toBe('wisp');
  expect(projectOfRow(row({ sum: { project: 'gone', cwd: '/tmp/x' } }), ps)).toBeUndefined();
});

// Audit D #1 / E: a task asking through runtime.ask arrives as `state: "waiting"` with an empty `question`; Home's
// five left it out (sorted by time only) while the sidebar listed it first. v1 split.js paintHome: groups().needs.
test('a_task_asking_through_runtime_ask_counts_as_asking_and_comes_first_on_home', () => {
  const ask = row({ id: 'ask1', state: 'waiting', updated_at: 1, sum: { question: '', approval: false } });
  expect(asking(ask)).toBe(true);
  expect(asking(row({ id: 'r', sum: { approval: true } }))).toBe(true);
  expect(asking(row({ id: 'r' }))).toBe(false);
  const fillers = [2, 3, 4, 5, 6, 7].map((n) => row({ id: `f${n}`, updated_at: n }));
  const ids = homeNeeds([...fillers, ask]).map((s) => s.id);
  expect(ids.slice(0, 2)).toEqual(['ask1', 'f7']);
});
