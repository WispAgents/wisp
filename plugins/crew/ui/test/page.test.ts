import { expect, test } from 'vitest';
import { jobOf } from '../src/jobs.ts';
import { againPrompts, facts, folderChip, heroStats, pillOf, quietLabel, shortDuration } from '../src/page.ts';
import { t } from '../src/i18n.ts';
import type { Member, SessionRow } from '../src/types.ts';

const say = t as never;

test('a_members_page_reads_its_jobs_from_the_peers_task_rows_and_says_how_it_works', () => {
  // runtime's row, the crew's note in body.ext
  const row = {
    id: 's1',
    title: 'Review #12',
    state: 'review',
    agent: 'claude',
    archived: false,
    unread: true,
    created_at: 10,
    updated_at: 20,
    summary: { status: 'done', preview: 'Two nits', live: '', cwd: '/w/web' },
    body: { ext: { 'wisp.crew': { id: 'ada', job: 'task', prompt: 'review #12' } } },
  };
  const j = jobOf(row);
  expect(j).toMatchObject({ id: 's1', state: 'review', createdAt: 10, updatedAt: 20, cwd: '/w/web', preview: 'Two nits', unread: true, crew: { id: 'ada', job: 'task', prompt: 'review #12' } });
  // a task nobody's crew works on
  expect(jobOf({ id: 's2', body: { ext: {} } })).toBeUndefined();
  expect(jobOf({ id: 's3' })).toBeUndefined();
  const m = { cron: '', on: [], parallel: 2, cwd: '/w/web', machine: '', agent: 'claude', mode: 'auto', team: [] } as unknown as Member;
  expect(facts(m, { t: say, harness: () => ({ name: 'Claude Code' }), place: (x) => x }).map((f) => f.text)).toEqual([
    'Only when asked',
    '2 jobs at once',
    'web · This computer',
    'Claude Code · Full auto',
    'No delegates',
  ]);
  expect(pillOf(j!, say)).toEqual({ label: 'Done', tone: 'good' });
  expect(pillOf({ ...j!, state: 'waiting' }, say)?.label).toBe('Asking you');
  expect(
    quietLabel(
      [
        { ...j!, crew: { id: 'ada', job: 'checkin' } },
        { ...j!, crew: { id: 'ada', job: 'checkin' } },
      ],
      say,
    ),
  ).toBe('2 check-ins · nothing new');
});

test('the_hero_shows_only_numbers_its_jobs_back_up', () => {
  const now = 100 * 86_400_000;
  const job = (o: Partial<SessionRow>): SessionRow => ({
    id: o.id ?? 'x',
    title: '',
    agent: 'claude',
    cwd: '',
    state: 'review',
    createdAt: now - 1000,
    updatedAt: now,
    ...o,
  });
  // a new member: nothing to show
  expect(heroStats({ team: [] }, [], { t: say, now })).toEqual([]);
  const jobs = [
    job({
      id: 'a',
      startedAt: 1000,
      endedAt: 601_000,
      crew: { id: 'ada', job: 'task', prompt: 'review #12' },
    }),
    job({
      id: 'b',
      startedAt: 1000,
      endedAt: 1_081_000,
      status: 'failed',
      crew: { id: 'ada', job: 'task', prompt: 'review #12' },
    }),
    job({
      id: 'c',
      state: 'running',
      createdAt: now - 10 * 86_400_000,
      crew: { id: 'ada', job: 'checkin' },
    }),
    job({
      id: 'd',
      createdAt: now - 20 * 86_400_000,
      crew: { id: 'ada', job: 'task', prompt: 'summarise the week' },
    }),
  ];
  // failed and running jobs aren't done; the old ones aren't this week; a run without both ends has no length
  expect(heroStats({ team: [{ name: 'Linus', agent: '', model: '', effort: '' }] }, jobs, { t: say, now })).toEqual([
    { value: '2', label: 'jobs done' },
    { value: '2', label: 'this week' },
    { value: '14m', label: 'a job takes' },
    { value: 'Linus', label: 'hands work to' },
  ]);
  expect(
    heroStats(
      {
        team: [
          { name: 'a', agent: '', model: '', effort: '' },
          { name: 'b', agent: '', model: '', effort: '' },
        ],
      },
      [],
      { t: say, now },
    ),
  ).toEqual([{ value: '2 helpers', label: 'hands work to' }]);
  // your prompts, newest first, each once; check-ins aren't yours
  expect(againPrompts(jobs)).toEqual(['review #12', 'summarise the week']);
  expect(againPrompts(jobs, 1)).toEqual(['review #12']);
  expect([shortDuration(40_000), shortDuration(14 * 60_000), shortDuration(125 * 60_000), shortDuration(120 * 60_000)]).toEqual(['40s', '14m', '2h 5m', '2h']);
  expect(folderChip({ cwd: '/w/web', dirs: ['/w/api'], machine: 'studio' }, { t: say, place: (x) => x.toUpperCase() })).toBe('web · +1 folder · STUDIO');
  expect(folderChip({ cwd: '', machine: '' }, { t: say, place: (x) => x })).toBe('No folder');
});
