// The crew's stories on phones (ADR-0065 step 13b): each ring's state from the member's jobs, the rings' order (waiting
// on you, then an unread report, then working, then idle), stepping through the stories, what the story shows and
// where its reply goes (a follow-up on the running job, else a new job).
import { expect, test } from 'vitest';
import { memberState } from '../src/rules/feed.ts';
import { t } from '../src/i18n.ts';
import type { State } from '../src/model.ts';
import { askJob, lastReport, nextLine, nowJob, replyTarget, ringLabel, ringOf, stepStory, storyOrder } from '../src/stories.ts';
import type { SessionRow } from '../src/types.ts';
import { orgOpen, orgParams } from '../src/org-params.ts';

const say = t as never;
const job = (id: string, crew: string, extra: Partial<SessionRow> = {}): SessionRow =>
  ({ id, title: id, agent: 'claude', cwd: '', state: 'running', createdAt: 1, updatedAt: 1, crew: { id: crew, job: 'task' }, ...extra }) as SessionRow;
const stateOf = (id: string, jobs: SessionRow[], paused = false): State => memberState({ id, paused }, jobs);

test('ring_state_comes_from_the_members_jobs_ask_then_unread_report_then_working_then_idle', () => {
  expect(ringOf(stateOf('a', [job('j1', 'a', { state: 'waiting' }), job('j2', 'a')]))).toBe('wait');
  expect(ringOf(stateOf('a', [job('j1', 'a', { state: 'review', unread: true }), job('j2', 'a')]))).toBe('report');
  expect(ringOf(stateOf('a', [job('j1', 'a')]))).toBe('work');
  // a job still starting up is work too (the Starting up card)
  expect(ringOf(stateOf('a', [job('j1', 'a', { pending: true })]))).toBe('work');
  // a report you read, or one saved for later, is no ping
  expect(ringOf(stateOf('a', [job('j1', 'a', { state: 'review', unread: false })]))).toBe('idle');
  expect(ringOf(stateOf('a', [job('j1', 'a', { state: 'review', unread: true, forLater: { at: 1 } })]))).toBe('idle');
  expect(ringOf(stateOf('a', []))).toBe('idle');
  expect(ringLabel('Ada', 'wait', say)).toBe('Ada, waiting on you');
});

test('ring_order_is_waiting_then_unread_report_then_working_then_idle_keeping_the_crews_rank_within_each', () => {
  const ranked = ['idle1', 'work1', 'report', 'wait', 'idle2', 'work2'].map((id) => ({ id }));
  const states = new Map<string, State>([
    ['idle1', stateOf('idle1', [])],
    ['work1', stateOf('work1', [job('a', 'work1')])],
    ['report', stateOf('report', [job('b', 'report', { state: 'review', unread: true })])],
    ['wait', stateOf('wait', [job('c', 'wait', { state: 'waiting' })])],
    ['idle2', stateOf('idle2', [])],
    ['work2', stateOf('work2', [job('d', 'work2')])],
  ]);
  expect(storyOrder(ranked, states).map((m) => m.id)).toEqual(['wait', 'report', 'work1', 'work2', 'idle1', 'idle2']);
  // a member the states don't know yet is idle, never dropped
  expect(storyOrder([{ id: 'new' }, ...ranked], states).map((m) => m.id).at(-1)).toBe('idle2');
  expect(storyOrder([{ id: 'new' }], new Map()).map((m) => m.id)).toEqual(['new']);
});

test('story_steps_stop_at_either_end', () => {
  expect(stepStory(0, 1, 3)).toBe(1);
  expect(stepStory(2, 1, 3)).toBe(2);
  expect(stepStory(0, -1, 3)).toBe(0);
  expect(stepStory(1, -1, 3)).toBe(0);
  expect(stepStory(0, 1, 0)).toBe(0);
});

test('the_story_shows_the_ask_the_running_job_and_the_last_report_unread_first', () => {
  const st = stateOf('a', [
    job('old', 'a', { state: 'done', updatedAt: 5, preview: 'old report' }),
    job('unread', 'a', { state: 'review', unread: true, updatedAt: 3 }),
    job('ask', 'a', { state: 'waiting', updatedAt: 9 }),
    job('run', 'a', { updatedAt: 8 }),
  ]);
  expect(askJob(st)?.id).toBe('ask');
  expect(nowJob(st)?.id).toBe('run');
  expect(lastReport(st)?.id).toBe('unread');
  expect(lastReport(stateOf('a', [job('old', 'a', { state: 'done', updatedAt: 5 }), job('new', 'a', { state: 'review', updatedAt: 7 })]))?.id).toBe('new');
  // only asking: what it's on now is the job asking you
  expect(nowJob(stateOf('a', [job('ask', 'a', { state: 'waiting' })]))?.id).toBe('ask');
  expect(nowJob(stateOf('a', []))).toBeUndefined();
  expect(lastReport(stateOf('a', [job('run', 'a')]))).toBeUndefined();
});

test('the_reply_goes_to_the_running_job_and_starts_a_job_when_the_member_is_idle', () => {
  const running = replyTarget(stateOf('a', [job('done', 'a', { state: 'done' }), job('run', 'a')]));
  expect(running).toEqual({ kind: 'follow', job: expect.objectContaining({ id: 'run' }) });
  // asking you and nothing running: the message goes to the job that asks
  expect(replyTarget(stateOf('a', [job('ask', 'a', { state: 'waiting' })]))).toEqual({ kind: 'follow', job: expect.objectContaining({ id: 'ask' }) });
  // idle, a report waiting, or a job still starting (no task yet to follow up on): a new job
  expect(replyTarget(stateOf('a', []))).toEqual({ kind: 'assign' });
  expect(replyTarget(stateOf('a', [job('r', 'a', { state: 'review', unread: true })]))).toEqual({ kind: 'assign' });
  expect(replyTarget(stateOf('a', [job('p', 'a', { pending: true })]))).toEqual({ kind: 'assign' });
});

test('the_next_card_says_when_it_looks_again_else_its_schedule_and_paused_says_so', () => {
  const c = { t: say, now: 100, when: () => '18:00' };
  expect(nextLine({ cron: '', on: [], paused: false, wakeAt: 200 }, c)).toBe('Check-in at 18:00');
  expect(nextLine({ cron: '0 * * * *', on: [], paused: false, wakeAt: null }, c)).toBe(t('wisp.crew.cadence.hourly'));
  expect(nextLine({ cron: '0 * * * *', on: [], paused: true, wakeAt: 200 }, c)).toBe('Paused');
});

// QA round 2 #3: the Org bubble opened the Crew view bare, which on a phone is the members list; the chart is the Org
// view with `{ chart: true }` (the Crew tab's own Org button). The browser draws the chart either way.
test('the_org_bubble_opens_the_org_chart', () => {
  expect(orgOpen()).toEqual({ t: 'openView', id: 'crew.org', params: { chart: true } });
  expect(orgParams(orgOpen().params)).toEqual({ chart: true });
});
