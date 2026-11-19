// New task's Send, later (src/launch.ts, v1 launcher.js go → planAsk, when.js): the clock, a time in the prompt, /draft, /after.
import { expect, test } from 'vitest';
import { plannedSaid } from '../src/format.ts';
import { launchPlan } from '../src/launch.ts';
import later from '../src/later.ts';

const now = new Date(2026, 10, 12, 10, 0, 0);
const o = { now, title: (id: string) => (id === '0192f3a4-aaaa-7bbb-8ccc-1234567890ab' || id === 'local:s0001' ? 'Fix login' : undefined) };

test('a time the prompt starts with holds it until then; "remind me" makes a reminder_v1_prompt_time', () => {
  const p = launchPlan('in 5 min run the e2e suite', o);
  expect(p).toMatchObject({ kind: 'task', rest: 'run the e2e suite' });
  expect(p && 'at' in p ? p.at - now.getTime() : 0).toBe(5 * 60_000);
  expect(launchPlan('remind me to review the PR in 1 hour', o)).toMatchObject({ kind: 'reminder', rest: 'review the PR' });
  // a time mid-prompt is just words: nothing is delayed on a guess
  expect(launchPlan('make the cron run at 2pm', o)).toBeNull();
});

test('a time picked on the clock holds the prompt as it is_v1_clock_pick', () => {
  const at = now.getTime() + 3_600_000;
  expect(launchPlan('bump the deps', { ...o, at })).toEqual({ kind: 'task', at, rest: 'bump the deps' });
  expect(launchPlan('bump the deps', { ...o, at: now.getTime() - 1 })).toBeNull(); // gone by while typing: starts now
});

test('/draft keeps it in Drafts; /after a task starts it once that one is done_v1_draft_after', () => {
  expect(launchPlan('/draft fix the login bug', o)).toEqual({ kind: 'task', draft: true, rest: 'fix the login bug' });
  expect(launchPlan('/draft', o)).toBeNull();
  expect(launchPlan('/after @[Fix login](wisp:local:s0001) write the release notes', o)).toEqual({ kind: 'task', after: { id: 'local:s0001', title: 'Fix login' }, rest: 'write the release notes' });
  expect(launchPlan('/after 0192f3a4-aaaa-7bbb-8ccc-1234567890ab write the notes', o)).toMatchObject({ after: { id: '0192f3a4-aaaa-7bbb-8ccc-1234567890ab', title: 'Fix login' } });
  expect(launchPlan('/after wisp:nope123 write the notes', o)).toBeNull(); // no such task here
});

// B54: Settings → Read times in prompts off: a time typed at the start stays words; a picked time still schedules
test('with read times off a typed time is not read, a picked one still is', () => {
  const now = new Date('2026-11-17T10:00:00Z');
  const base = { now, title: () => undefined };
  expect(launchPlan('in 5 min run the e2e suite', { ...base, readTimes: false })).toBeNull();
  expect(launchPlan('in 5 min run the e2e suite', { ...base, readTimes: true })).toMatchObject({ kind: 'task' });
  expect(launchPlan('run it', { ...base, readTimes: false, at: now.getTime() + 60_000 })).toMatchObject({ kind: 'task', at: now.getTime() + 60_000 });
});

// Audit B #6: Start later made the plan, in Scheduled below a long Needs you, and nothing on screen said so. New task
// says where it went, as v1 did ("Starts 03:56"; planSaid for /draft and /after).
test('send_says_where_a_planned_task_went_as_v1_did', () => {
  const source = later.prompt?.find((s) => s.id === 'later.launch');
  const said: string[] = [];
  const ran: unknown[] = [];
  const client = {
    home: 'local',
    entities: { peek: () => undefined },
    run: (_c: unknown, a: unknown) => (ran.push(a), Promise.resolve({})),
  };
  const at = Date.now() + 10 * 60_000;
  const took = source?.send?.('bump the deps', { client: client as never, task: '', launch: { at } as never, say: (m) => said.push(m) });
  expect(took).toBe(true);
  expect(ran).toHaveLength(1);
  expect(said).toHaveLength(1);
  expect(said[0]).toMatch(/^Starts (\w{3} )?\d\d:\d\d$/);
  expect(plannedSaid({ kind: 'task', draft: true }, 0)).toBe('Saved to Drafts');
  expect(plannedSaid({ kind: 'task', after: { title: 'Fix login' } }, 0)).toBe('Starts once Fix login is done');
});
