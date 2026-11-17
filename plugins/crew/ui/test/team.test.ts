import { expect, test } from 'vitest';
import { hasTeam, shownOf, teamRows, timeOf, type Say } from '../src/team.ts';
import { machinesByPeer } from '../src/data.ts';
import { jobOf } from '../src/jobs.ts';
import type { SessionRow } from '../src/types.ts';

const say: Say = (k) => k;
const job = (id: string, crew: string, kind: 'delegated' | 'mention' | 'task' | 'checkin' | 'event', from: string | undefined, more: Partial<SessionRow> = {}): SessionRow => ({
  id,
  title: `${id} title`,
  agent: 'claude',
  cwd: '',
  state: 'done',
  createdAt: Number(id.slice(1)),
  updatedAt: 0,
  crew: { id: crew, job: kind, ...(from && { from: { id: from, title: 'Lead' } }) },
  ...more,
});
const members = new Map([
  ['ada', { id: 'ada', name: 'Ada', avatar: 'x', color: 'violet' }],
  ['bo', { id: 'bo', name: 'Bo' }],
]);

test('the_leads_team_panel_lists_the_crew_jobs_it_handed_out_oldest_first_each_opening_its_job', () => {
  const jobs = [
    job('j3', 'bo', 'mention', 'lead1', { state: 'running', live: 'Reading src/' }),
    job('j1', 'ada', 'delegated', 'lead1', { preview: 'Fixed the flaky test' }),
    job('j2', 'ada', 'task', 'lead1', { preview: 'Answered' }), // asked from the lead's reply box (crew.ask)
    job('j7', 'ada', 'event', 'lead1'), // a check on the task: not work it handed out
    job('j8', 'ada', 'task', undefined), // sent from Ada's page
    job('j4', 'bo', 'delegated', 'lead2'), // another task's
    job('j5', 'ada', 'delegated', undefined),
    job('j6', 'gone', 'delegated', 'lead1', { state: 'waiting' }),
  ];
  // the task key is `peer:id`; the job's `from` is the bare id
  const rows = teamRows(jobs, 'me:lead1', (id) => members.get(id), say);
  expect(rows.map((r) => [r.id, r.handle, r.line, r.status, r.live])).toEqual([
    ['j1', '@Ada', 'Fixed the flaky test', 'wisp.crew.team.done', false],
    ['j2', '@Ada', 'Answered', 'wisp.crew.team.done', false],
    ['j3', '@Bo', 'Reading src/', 'wisp.crew.job.working', true],
    ['j6', 'wisp.crew.team.someone', 'j6 title', 'wisp.crew.job.asking', false],
  ]);
  expect(hasTeam(jobs, 'me:lead1')).toBe(true);
  expect(hasTeam(jobs, 'me:lead9')).toBe(false);
});

test('a_delegated_job_still_being_made_shows_starting_and_a_failed_one_says_so', () => {
  const rows = teamRows(
    [job('j1', 'ada', 'delegated', 'L', { pending: true, state: 'running' }), job('j2', 'ada', 'delegated', 'L', { status: 'error', state: 'review' })],
    'L',
    (id) => members.get(id),
    say,
  );
  expect(rows.map((r) => [r.status, r.tone, r.pending])).toEqual([
    ['wisp.crew.job.starting', 'accent', true],
    ['wisp.crew.team.failed', 'bad', false],
  ]);
});

test('each_team_row_says_where_its_job_runs_this_computer_or_the_members_machine_v1_tl_place', () => {
  // v1 team.js ranOn: this computer (a laptop) with the lead; a job on Ada's machine is that machine's row, its name on
  // a server chip, away from the lead (accent icon); the tooltip says where it ran, in the member's own folder
  const here = 'laptop';
  const jobs = [
    job('j1', 'ada', 'delegated', 'L', { state: 'running', startedAt: 1_000 }),
    job('j2', 'ada', 'delegated', 'L', { state: 'running', peer: 'studio', machine: 'Studio', startedAt: 2_000 }),
    job('j3', 'bo', 'delegated', 'L', { peer: 'gone' }),
  ];
  const rows = teamRows(jobs, `${here}:L`, (id) => members.get(id), say, here);
  expect(rows.map((r) => [r.id, r.place.name, r.place.icon, r.place.away])).toEqual([
    ['j1', 'wisp.crew.page.here', 'laptop', false],
    ['j2', 'Studio', 'server', true],
    ['j3', 'wisp.crew.team.elsewhere', 'server', true],
  ]);
  expect(rows[1]?.place.tip).toBe('wisp.crew.team.ranOn');
});

test('a_far_job_shows_its_machine_read_from_the_machines_peer_row_its_key_names', () => {
  // the data path: the far row comes keyed by the machine's peer id; wisp.machines' `machines_peer` row names it
  expect(machinesByPeer([{ id: 'laptop:studio', body: { peer: 'studio', name: 'Studio' } }, { id: 'laptop:off', body: { peer: null, name: 'Off' } }])).toEqual(new Map([['studio', 'Studio']]));
  // a lead on Studio and its job beside it there: on Studio (a server), not away
  const [r] = teamRows([job('j1', 'ada', 'delegated', 'L', { peer: 'studio', machine: 'Studio' })], 'studio:L', (id) => members.get(id), say, 'laptop');
  expect([r?.place.name, r?.place.icon, r?.place.away]).toEqual(['Studio', 'server', false]);
  // v1 ranOn: away from its lead is a server even here (the lead on Studio, the job on this computer)
  const [home] = teamRows([job('j2', 'ada', 'delegated', 'L', { peer: 'laptop' })], 'studio:L', (id) => members.get(id), say, 'laptop');
  expect([home?.place.name, home?.place.icon, home?.place.away]).toEqual(['wisp.crew.page.here', 'server', true]);
});

test('a_running_jobs_time_ticks_from_its_runs_start_and_a_finished_one_says_how_long_it_took_v1_elapsed', () => {
  // runtime's row says when the last run started and ended (summary.startedAt / endedAt)
  const row = jobOf({ id: 's1', created_at: 1, summary: { startedAt: 1_000, endedAt: 48_000 }, body: { ext: { 'wisp.crew': { id: 'ada', job: 'delegated', from: { id: 'L', title: 'Lead' } } } } });
  expect([row?.startedAt, row?.endedAt]).toEqual([1_000, 48_000]);
  expect(timeOf({ running: true, pending: false, startedAt: 1_000 }, 135_000)).toBe(134_000);
  expect(timeOf({ running: false, pending: false, startedAt: 1_000, endedAt: 48_000 }, 999_999)).toBe(47_000);
  expect(timeOf({ running: true, pending: true, startedAt: 1_000 }, 5_000)).toBeUndefined();
  expect(timeOf({ running: false, pending: false }, 5_000)).toBeUndefined();
  // a job asking you: its run still goes (v1 counts the turn as running), so its time ticks beside the Asks pill
  const [asking] = teamRows([job('j1', 'ada', 'delegated', 'L', { state: 'waiting', startedAt: 1_000 })], 'L', (id) => members.get(id), say);
  expect([asking?.live, asking?.running, asking && timeOf(asking, 61_000)]).toEqual([false, true, 60_000]);
});

// v1 c33f33e6: a long team shows 3 (the running ones first, newest first, then the newest back) and folds the rest
test('a long team shows three, running first, and never folds a single row', () => {
  const r = (running: boolean) => ({ running });
  expect(shownOf([r(false), r(false), r(false), r(false)])).toBeUndefined();
  expect([...(shownOf([r(false), r(true), r(false), r(false), r(false)]) ?? [])].sort()).toEqual([1, 3, 4]);
  expect([...(shownOf([r(true), r(true), r(true), r(true), r(false)]) ?? [])].sort()).toEqual([1, 2, 3]);
});

// Owner's desktop test 2026-11-17 #12: runtime's summary says a saved task's forLater as `{at, by}` (host ops.rs
// for_later), never `true`, so a job saved for later never read as saved.
test('a_job_the_server_says_is_saved_for_later_reads_as_saved_with_its_time', () => {
  const ext = { ext: { 'wisp.crew': { id: 'ada', job: 'delegated' } } };
  expect(jobOf({ id: 's1', created_at: 1, summary: { forLater: { at: 77, by: 'you' } }, body: ext })?.forLater).toEqual({ at: 77, by: 'you' });
  expect(jobOf({ id: 's1', created_at: 1, updated_at: 9, summary: { forLater: true }, body: ext })?.forLater).toEqual({ at: 9 });
  expect(jobOf({ id: 's1', created_at: 1, summary: {}, body: ext })?.forLater).toBeUndefined();
});
