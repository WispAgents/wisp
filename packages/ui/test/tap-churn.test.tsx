// F-phone-8: a tap on a phone's list (a row → its task page, Back → the list) changes a few attributes; nothing it
// doesn't show may be built again for it. Each test fails on the code before the fix.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createSignal, type JSX } from 'solid-js';
import { mount } from './helpers.tsx';
import { ListRow, Stage } from '../src/index.ts';
import { futureLabel, timeTitle } from '../src/content/time-format.ts';

/** A cell that counts how many times it was built. */
function counted(): { readonly n: () => number; readonly Cell: () => JSX.Element } {
  let n = 0;
  return {
    n: () => n,
    Cell: () => {
      n++;
      return <span>cell</span>;
    },
  };
}

describe('a tap builds nothing it does not show', () => {
  afterEach(() => vi.restoreAllMocks());

  it('a_feed_row_builds_its_time_and_foot_once_as_it_turns_current', () => {
    const time = counted();
    const foot = counted();
    const [current, setCurrent] = createSignal(false);
    const host = mount(() => <ListRow layout="feed" title="A task" time={<time.Cell />} foot={<foot.Cell />} current={current()} onOpen={() => {}} />);
    // the row's attributes say the cells are there, and the cells show: each built once
    const row = host.querySelector('[data-kit="list-row"]');
    expect(row?.hasAttribute('data-time')).toBe(true);
    expect(row?.hasAttribute('data-foot')).toBe(true);
    expect(time.n()).toBe(1);
    expect(foot.n()).toBe(1);
    // a tap opens it, Back leaves it: the row's attributes change, its cells don't
    for (const on of [true, false, true, false]) setCurrent(on);
    expect(row?.getAttribute('aria-current')).toBeNull();
    expect(time.n()).toBe(1);
    expect(foot.n()).toBe(1);
  });

  it('a_page_flip_builds_the_rail_once_and_says_it_on_the_panes', () => {
    const rail = counted();
    const [page, setPage] = createSignal<'list' | 'main'>('list');
    const [behind, setBehind] = createSignal(true);
    const host = mount(() => (
      <Stage layout="stack" page={page()} listBehind={behind()} rail={<rail.Cell />} head={<span>head</span>} list={<span>list</span>} main={<span>main</span>} gripLabel="Resize" />
    ));
    const pane = (kit: string): Element | null => host.querySelector(`[data-kit="${kit}"]`);
    expect(pane('split-main')?.hasAttribute('data-pane-off')).toBe(true);
    expect(pane('feed-wrap')?.hasAttribute('data-pane-off')).toBe(false);
    // a task's page: the list and its bar are off (kept laid out, skipped), never inert (content-visibility keeps
    // them out of Tab and the accessibility tree; toggling inert restyled the whole list)
    setBehind(false);
    setPage('main');
    expect(pane('split-main')?.hasAttribute('data-pane-off')).toBe(false);
    expect(pane('feed-wrap')?.hasAttribute('data-pane-off')).toBe(true);
    expect(pane('topbar')?.hasAttribute('data-pane-off')).toBe(true);
    expect(pane('feed-wrap')?.hasAttribute('data-pane-under')).toBe(false);
    expect(pane('feed-wrap')?.hasAttribute('inert')).toBe(false);
    // Home's page over the list: the list is drawn behind it, inert
    setBehind(true);
    expect(pane('feed-wrap')?.hasAttribute('data-pane-under')).toBe(true);
    expect(pane('feed-wrap')?.hasAttribute('inert')).toBe(true);
    for (const p of ['list', 'main', 'list'] as const) setPage(p);
    // the rail (never shown on a stack) isn't built there at all, not once per flip to test whether there is one
    expect(rail.n()).toBe(0);
  });

  it('the_split_rail_is_built_once_and_a_pane_going_off_keeps_no_focus', () => {
    const rail = counted();
    const [page, setPage] = createSignal<'list' | 'main'>('list');
    const [layout, setLayout] = createSignal<'split' | 'stack'>('split');
    const host = mount(() => (
      <Stage layout={layout()} page={page()} rail={<rail.Cell />} head={<input data-x="search" />} list={<button data-x="row">row</button>} main={<span>main</span>} gripLabel="Resize" />
    ));
    for (const p of ['main', 'list', 'main'] as const) setPage(p);
    expect(rail.n()).toBe(1);
    expect(host.querySelector('[data-kit="stage"]')?.hasAttribute('data-rail')).toBe(true);
    // a phone: the row just tapped and the search box lose the focus as their pane goes off
    setLayout('stack');
    setPage('list');
    for (const x of ['row', 'search']) {
      host.querySelector<HTMLElement>(`[data-x="${x}"]`)?.focus();
      expect(document.activeElement?.getAttribute('data-x')).toBe(x);
      setPage('main');
      expect(document.activeElement?.getAttribute('data-x')).toBeNull();
      setPage('list');
    }
  });

  it('each_date_format_is_made_once_per_locale_and_style', () => {
    const Real = Intl.DateTimeFormat;
    let made = 0;
    // counts each one made (a spy on the constructor wouldn't construct)
    vi.spyOn(Intl, 'DateTimeFormat').mockImplementation(function (...args: ConstructorParameters<typeof Intl.DateTimeFormat>) {
      made++;
      return new Real(...args);
    } as unknown as typeof Intl.DateTimeFormat);
    const t = ((k: string) => k) as Parameters<typeof timeTitle>[2];
    const now = Date.UTC(2026, 10, 16, 9);
    const a = timeTitle(now - 60_000, now, t, 'fr-CH');
    for (let i = 0; i < 20; i++) expect(timeTitle(now - 60_000, now, t, 'fr-CH')).toBe(a);
    for (let i = 0; i < 5; i++) futureLabel(now + 3_600_000, now, 'fr-CH');
    // one for the full date, one for the clock
    expect(made).toBe(2);
    expect(a).toBe(new Real('fr-CH', { dateStyle: 'medium', timeStyle: 'short' }).format(now - 60_000));
  });
});
