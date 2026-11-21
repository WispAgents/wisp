// The Crew tab in the phone apps (ADR-0065 step 14): what each member's row says, the Jobs tab's rows (newest first),
// the Wakes tab's lines, the menus (every v1 crew action reachable: F-crew-1, 3, 6, 7, 12, 13, 14) and Org's params.
import { expect, test } from 'vitest';
import { memberState } from '../src/rules/feed.ts';
import { t } from '../src/i18n.ts';
import { orgParams } from '../src/org-params.ts';
import { isTab, jobRows, memberMenu, memberRow, rootMenu, TABS, wakeLines } from '../src/phone.ts';
import type { Member, SessionRow } from '../src/types.ts';
import '../src/i18n/lazy.ts';

const job = (id: string, extra: Partial<SessionRow> = {}): SessionRow =>
  ({
    id,
    title: id,
    agent: 'claude',
    cwd: '',
    state: 'running',
    createdAt: 1,
    updatedAt: 1,
    crew: { id: 'a', job: 'task' },
    ...extra,
  }) as SessionRow;
const member = (extra: Partial<Member> = {}): Member => ({
  id: 'a',
  name: 'Ada',
  role: 'Reviewer',
  avatar: '',
  color: '',
  brief: '',
  cwd: '',
  machine: '',
  sandbox: '',
  agent: '',
  model: '',
  effort: '',
  mode: 'auto',
  cron: '',
  on: [],
  parallel: 1,
  kit: [],
  team: [],
  paused: false,
  notebook: '',
  wakeAt: null,
  checkins: 0,
  lastCheckinAt: null,
  lastMinute: null,
  offers: [],
  createdAt: 0,
  updatedAt: 0,
  ...extra,
});
const ctx = { t, now: 10, when: () => '09:00' };

test('member_row_says_role_and_what_its_on_with_a_pill_for_asks_working_and_new_report', () => {
  const ask = memberRow(member(), memberState({ id: 'a', paused: false }, [job('j', { state: 'waiting', question: 'Merge?' })]), ctx);
  expect(ask.sub.startsWith('Reviewer · ')).toBe(true);
  expect(ask.pill?.tone).toBe('warn');
  expect(memberRow(member(), memberState({ id: 'a', paused: false }, [job('j')]), ctx).pill?.tone).toBe('info');
  expect(memberRow(member(), memberState({ id: 'a', paused: false }, [job('j', { state: 'review', unread: true })]), ctx).pill?.label).toBe('New report');
  expect(memberRow(member(), memberState({ id: 'a', paused: false }, []), ctx).pill).toBeUndefined();
});

test('jobs_tab_lists_newest_first_with_what_it_asks_or_found', () => {
  const rows = jobRows([job('old', { createdAt: 1, state: 'review', preview: 'Found 2 bugs' }), job('new', { createdAt: 5, state: 'waiting', question: 'Merge?' })], { t });
  expect(rows.map((r) => r.id)).toEqual(['new', 'old']);
  expect(rows[0]?.sub).toBe('Merge?');
  expect(rows[1]?.sub).toBe('Found 2 bugs');
});

test('wakes_tab_says_when_it_checks_in_what_else_starts_it_and_the_next_check_in', () => {
  const lines = wakeLines(member({ cron: '0 * * * *', on: ['ci'], checkins: 3, wakeAt: 99 }), ctx);
  expect(lines.map((l) => l.id)).toEqual(['cadence', 'events', 'next', 'checkins']);
  expect(lines.find((l) => l.id === 'checkins')?.sub).toBe('3 check-ins');
  expect(lines.find((l) => l.id === 'next')?.sub).toContain('09:00');
  expect(wakeLines(member(), ctx).find((l) => l.id === 'events')?.sub).toBe('Nothing else');
});

test('every_v1_crew_action_is_in_a_menu_on_the_phone', () => {
  const values = (xs: readonly object[]): string[] => xs.flatMap((x) => ('value' in x && typeof x.value === 'string' ? [x.value] : []));
  expect(values(memberMenu(member(), { t }))).toEqual(['edit', 'checkin', 'pause', 'org', 'share', 'shareCrew', 'record', 'remove']);
  expect(values(memberMenu(member({ paused: true }), { t })).includes('pause')).toBe(true);
  expect(values(rootMenu({ t }, true))).toEqual(['import', 'shareCrew', 'record']);
  expect(values(rootMenu({ t }, false))).toEqual(['import']);
  expect(TABS).toEqual(['jobs', 'brief', 'wakes']);
  expect(isTab('brief') && !isTab('nope')).toBe(true);
});

test('org_params_open_the_chart_only_when_asked', () => {
  expect(orgParams(null)).toBeNull();
  expect(orgParams({ chart: true })).toEqual({ chart: true });
  expect(orgParams({ chart: 'yes' })).toBeNull();
});
