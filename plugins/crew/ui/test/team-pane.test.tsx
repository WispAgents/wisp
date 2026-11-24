// The lead's Team tab (v1 team.js teamLiveHtml): each row's machine chip and its time, and a ticking clock that
// repaints only the time.
import { afterEach, expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import { TeamList } from '../src/TeamPane.tsx';
import type { TeamRow } from '../src/team.ts';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});

const row = (id: string, more: Partial<TeamRow> = {}): TeamRow => ({
  id,
  member: 'ada',
  name: 'Ada',
  avatar: '',
  color: 'violet',
  handle: '@Ada',
  line: 'Reading src/sync',
  status: 'Working',
  tone: 'accent',
  live: true,
  running: true,
  pending: false,
  place: { name: 'This computer', icon: 'laptop', away: false, tip: 'Ran on This computer' },
  startedAt: 0,
  ...more,
});

function mount(rows: readonly TeamRow[]) {
  const [now, setNow] = createSignal(134_000);
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <TeamList rows={rows} now={now} open={() => undefined} />, host);
  const q = (sel: string): HTMLElement[] => [...host.querySelectorAll<HTMLElement>(sel)];
  return { host, setNow, q };
}

test('team_rows_render_the_machine_chip_and_the_running_time_v1_tl_place_elapsed', () => {
  const { q } = mount([
    row('j1'),
    row('j2', { place: { name: 'Studio', icon: 'server', away: true, tip: 'Ran on Studio' }, startedAt: 87_000 }),
    row('j3', { live: false, running: false, status: 'Done', tone: 'good', startedAt: 0, endedAt: 47_000 }),
  ]);
  const chips = q('[data-part="crew.team.place"]');
  expect(chips.map((c) => [c.textContent, c.hasAttribute('data-away'), c.title])).toEqual([
    ['This computer', false, 'Ran on This computer'],
    ['Studio', true, 'Ran on Studio'],
    ['This computer', false, 'Ran on This computer'],
  ]);
  expect(chips.map((c) => c.querySelector('[data-kit="icon"]')?.getAttribute('data-icon'))).toEqual(['laptop', 'server', 'laptop']);
  // a running delegate shows its time, not a pill (v1 .tl-row); one that's back: its pill and how long it took
  expect(q('[data-part="crew.team.time"]').map((t) => t.textContent)).toEqual(['2m 14s', '47s', '47s']);
  expect(q('[data-kit="pill"]').map((p) => p.textContent)).toEqual(['Done']);
});

test('a_far_job_shows_its_machine_on_its_row', () => {
  const { q } = mount([row('j1', { place: { name: 'Studio', icon: 'server', away: true, tip: "Ran on Studio, in Ada's own folder" } })]);
  const chip = q('[data-part="crew.team.place"]')[0];
  expect(chip?.textContent).toBe('Studio');
  expect(chip?.title).toBe("Ran on Studio, in Ada's own folder");
  expect(chip?.hasAttribute('data-away')).toBe(true);
});

test('the_running_time_updates_without_re_rendering_the_list', () => {
  const { q, setNow } = mount([row('j1'), row('j2', { live: false, running: false, status: 'Done', tone: 'good', endedAt: 5_000 })]);
  const rowsBefore = q('[data-part~="crew.team.row"]');
  const timeBefore = q('[data-part="crew.team.time"]');
  expect(timeBefore.map((t) => t.textContent)).toEqual(['2m 14s', '5s']);
  setNow(135_000);
  const rowsAfter = q('[data-part~="crew.team.row"]');
  const timeAfter = q('[data-part="crew.team.time"]');
  expect(timeAfter.map((t) => t.textContent)).toEqual(['2m 15s', '5s']);
  // the same row and time nodes: only the time's text changed
  expect(rowsAfter.length).toBe(2);
  rowsAfter.forEach((r, i) => expect(r).toBe(rowsBefore[i]));
  timeAfter.forEach((t, i) => expect(t).toBe(timeBefore[i]));
});

test('a_working_job_with_no_start_to_tick_from_says_its_state_and_keeps_no_empty_time_column', () => {
  // a far wisp's older row has no `startedAt`: the pill says Working, and the row has no time cell at all
  const { q } = mount([row('j1', { startedAt: undefined as never })]);
  expect(q('[data-kit="pill"]').map((p) => p.textContent)).toEqual(['Working']);
  expect(q('[data-part="crew.team.time"]')).toEqual([]);
  expect(q('[data-part~="crew.team.row"]')[0]?.hasAttribute('data-time')).toBe(false);
});

test('updating_team_rows_patches_in_place_by_id_without_remounting_dom_nodes', () => {
  const [rows, setRows] = createSignal<TeamRow[]>([row('j1', { line: 'Reading a.ts' }), row('j2', { line: 'Running tests' })]);
  const [now] = createSignal(134_000);
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(() => <TeamList rows={rows()} now={now} open={() => undefined} />, host);
  const before = [...host.querySelectorAll('[data-part~="crew.team.row"]')];
  setRows([row('j1', { line: 'Editing b.ts' }), row('j2', { line: 'Running tests' })]);
  const after = [...host.querySelectorAll('[data-part~="crew.team.row"]')];
  expect(after.length).toBe(2);
  expect(after[0]).toBe(before[0]);
  expect(after[1]).toBe(before[1]);
  expect(after[0]?.textContent).toContain('Editing b.ts');
});
