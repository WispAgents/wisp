import { describe, expect, it } from 'vitest';
import { flatRow, noticeFor, notifyOn, NOTIFY_KEY } from '../src/notify.ts';

const t = (k: string, v?: Readonly<Record<string, string | number>>): string => `${k}${v ? JSON.stringify(v) : ''}`;
const row = (state: string, summary: Record<string, unknown> = {}) => ({ id: 's1', title: 'Fix login', state, archived: false, summary: { preview: 'done it', question: '', approval: false, ...summary } });

describe('notify', () => {
  it('a_system_notification_shows_only_with_settings_notifications_switch_on_v1_notify_js', () => {
    expect(NOTIFY_KEY).toBe('settings.notify');
    expect([notifyOn('on'), notifyOn(true)]).toEqual([true, true]);
    expect([notifyOn('off'), notifyOn(false), notifyOn(undefined), notifyOn(null)]).toEqual([false, false, false, false]);
  });
  it('a_task_that_finishes_or_asks_tells_you_once_from_the_stores_row_shape', () => {
    const running = flatRow(row('running'));
    const done = flatRow(row('review'));
    const asking = flatRow(row('review', { question: 'Which branch?' }));
    expect(running?.question).toBeUndefined();
    const fin = noticeFor(running, done!, { siblings: [], t });
    expect(fin?.head).toContain('shell-home.notify.ready');
    expect(fin?.body).toBe('done it');
    const ask = noticeFor(done, asking!, { siblings: [], t });
    expect(ask?.asks).toBe(true);
    expect(ask?.body).toBe('Which branch?');
    expect(noticeFor(asking, asking!, { siblings: [], t })).toBeNull();
    expect(noticeFor(done, done!, { siblings: [], t })).toBeNull();
  });

  it('a_crew_members_finished_job_says_who_reported_read_from_the_rows_crew_note', () => {
    const job = (state: string) => ({ ...row(state), body: { ext: { 'wisp.crew': { id: 'm1', job: 'task' } } } });
    const running = flatRow(job('running'));
    const done = flatRow(job('review'));
    expect(done?.crew).toEqual({ id: 'm1' });
    const n = noticeFor(running, done!, { siblings: [], t, member: (id) => (id === 'm1' ? 'Ada' : undefined) });
    expect(n?.head).toBe('shell-home.notify.member{"name":"Ada","title":"Fix login"}');
    expect(n?.title).toBe('Ada: Fix login');
    // a member who left: the job reads as any task's
    expect(noticeFor(running, done!, { siblings: [], t, member: () => undefined })?.head).toContain('shell-home.notify.ready');
  });

  // ADR-0114 (the owner's delegate, 2026-11-20): a factory's stations post nothing of their own; the run tells you when it
  // waits on you or ends
  it('a_factory_station_never_notifies_only_its_run_does', () => {
    const station = (state: string) => flatRow(row(state, { factoryStation: { run: 'r1', station: 'build' } }));
    expect(noticeFor(station('running'), station('review')!, { siblings: [], t })).toBeNull();
    const asks = flatRow(row('review', { factoryStation: { run: 'r1', station: 'build' }, question: 'May I?' }));
    // a station's agent asking you is the run waiting on you: that still tells you
    expect(noticeFor(station('running'), asks!, { siblings: [], t })?.asks).toBe(true);
    const run = (state: string, more: Record<string, unknown> = {}) => flatRow(row(state, { factory: { name: 'default', state: 'running', stations: [] }, ...more }));
    expect(noticeFor(run('running'), run('review')!, { siblings: [], t })?.head).toContain('shell-home.notify.ready');
    expect(noticeFor(run('running'), run('review', { question: 'Approve the spec?' })!, { siblings: [], t })?.asks).toBe(true);
  });
});
