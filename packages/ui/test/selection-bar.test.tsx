// ADR-0065 step 13 (design/mobile.md §A.4 Selection mode): the kit's selection bar in each platform's shape and words
// (Android's contextual top app bar that leaves with the last row unpicked; iOS's edit mode with Select All, Done and a
// bottom toolbar that stays until Done), a feed row's pick mark, and iOS's computer filter as a pull-down from the
// title (Android and the browser keep the title's own action).
import { describe, expect, it } from 'vitest';
import { createSignal, type JSX } from 'solid-js';
import { mount } from './helpers.tsx';
import { ListRow, PlatformLookProvider, SelectionBar, TopAppBar, type Look, type SelectionAction } from '../src/index.ts';

const noop = (): void => {};
const as = (look: Look, view: () => JSX.Element): HTMLElement => mount(() => <PlatformLookProvider look={() => look}>{view()}</PlatformLookProvider>);
const ACTIONS: SelectionAction[] = [
  { id: 'done', label: 'Mark done', icon: 'done', onSelect: noop },
  { id: 'delete', label: 'Delete', icon: 'trash', onSelect: noop, danger: true, disabled: true },
];
const visible = (host: ParentNode, sel: string): HTMLElement[] => [...host.querySelectorAll<HTMLElement>(sel)].filter((e) => !e.closest('[data-kit="toolbar-measure"]'));

describe('SelectionBar', () => {
  it('on_android_is_the_contextual_bar_with_close_the_count_and_the_actions_and_leaves_when_the_last_row_is_unpicked', () => {
    const [count, setCount] = createSignal(2);
    let left = 0;
    const host = as('android', () => <SelectionBar part="t.sel" count={count()} allPicked={false} onAll={noop} onExit={() => left++} actions={ACTIONS} actionsLabel="Selected tasks" />);
    expect(host.querySelector('[data-kit="app-bar"]')?.getAttribute('data-kind')).toBe('select');
    expect(host.querySelector('[data-part="t.sel.exit"]')?.getAttribute('aria-label')).toBe('Close selection');
    expect(host.querySelector('[data-part="t.sel.title"]')?.textContent).toBe('2 selected');
    expect(visible(host, '[data-part~="t.sel.actions"] button').length, 'Select all and the actions in the bar').toBeGreaterThan(0);
    expect(host.querySelector('[data-kit="select-toolbar"]'), 'no bottom toolbar').toBeNull();
    setCount(1);
    expect(left).toBe(0);
    setCount(0);
    expect(left, 'M3: nothing picked, the contextual bar goes').toBe(1);
  });

  it('on_ios_is_edit_mode_with_select_all_the_count_done_and_a_bottom_toolbar_that_stays_with_nothing_picked', () => {
    const [count, setCount] = createSignal(0);
    const [all, setAll] = createSignal(false);
    let left = 0;
    const host = as('ios', () => <SelectionBar part="t.sel" count={count()} allPicked={all()} onAll={noop} onExit={() => left++} actions={ACTIONS} actionsLabel="Selected tasks" />);
    expect(host.querySelector('[data-part="t.sel.title"]')?.textContent).toBe('Select Items');
    expect(host.querySelector('[data-part="t.sel.all"]')?.textContent).toBe('Select All');
    expect(host.querySelector('[data-kit="app-bar-confirm"] [data-part="t.sel.exit"]')?.textContent).toBe('Done');
    expect(host.querySelector('[data-kit="select-toolbar"] [data-kit="toolbar"]'), 'the bulk actions at the bottom').not.toBeNull();
    setCount(3);
    setAll(true);
    expect(host.querySelector('[data-part="t.sel.title"]')?.textContent).toBe('3 Selected');
    expect(host.querySelector('[data-part="t.sel.all"]')?.textContent).toBe('Deselect All');
    setCount(0);
    expect(left, 'edit mode waits for Done').toBe(0);
    host.querySelector<HTMLButtonElement>('[data-part="t.sel.exit"]')?.click();
    expect(left).toBe(1);
  });
});

describe('a feed row in selection mode', () => {
  it('leads_with_its_pick_mark_in_place_of_its_lead_and_says_whether_it_is_picked', () => {
    const [picked, setPicked] = createSignal<boolean | undefined>(undefined);
    const host = as('ios', () => <ListRow layout="feed" title="Fix it" lead={<span data-test="dot" />} onOpen={noop} {...(picked() === undefined ? {} : { picked: picked() })} />);
    const row = host.querySelector('[data-kit="list-row"]');
    expect(host.querySelector('[data-test="dot"]')).not.toBeNull();
    expect(row?.hasAttribute('aria-pressed')).toBe(false);
    setPicked(false);
    expect(host.querySelector('[data-test="dot"]'), 'the check takes the lead’s place').toBeNull();
    expect(host.querySelector('[data-kit="row-pick"]')).not.toBeNull();
    expect(row?.getAttribute('aria-pressed')).toBe('false');
    setPicked(true);
    expect(row?.getAttribute('aria-pressed')).toBe('true');
    expect(row?.hasAttribute('data-picked')).toBe(true);
    expect(host.querySelector('[data-kit="row-pick"] svg[data-icon="check"]')).not.toBeNull();
  });

  it('draws_nothing_of_it_in_the_browser_when_unset', () => {
    const host = as('web', () => <ListRow layout="feed" title="Fix it" onOpen={noop} />);
    const row = host.querySelector('[data-kit="list-row"]');
    expect(row?.hasAttribute('data-picking') || row?.hasAttribute('aria-pressed') || host.querySelector('[data-kit="row-pick"]') !== null).toBe(false);
  });
});

describe('the title as a pull-down', () => {
  const MENU = { label: 'Show tasks from', items: [{ value: 'all', label: 'All', checked: true }], onSelect: noop };
  it('on_ios_the_title_opens_its_menu_and_on_android_it_keeps_its_own_action', () => {
    let tapped = 0;
    const ios = as('ios', () => <TopAppBar kind="root" part="t.bar" title="All" onTitle={() => tapped++} titleMenu={MENU} />);
    const inline = ios.querySelector<HTMLElement>('[data-part="t.bar.title"] [data-kit="app-bar-title-button"]');
    expect(inline?.getAttribute('aria-haspopup')).toBe('menu');
    expect(ios.querySelector('[data-kit="app-bar-large"] [data-kit="app-bar-title-button"]')?.getAttribute('aria-haspopup'), 'the large title too').toBe('menu');
    inline?.click();
    expect(tapped, 'not the sheet').toBe(0);
    const android = as('android', () => <TopAppBar kind="root" part="t.bar" title="All" onTitle={() => tapped++} titleMenu={MENU} />);
    const b = android.querySelector<HTMLElement>('[data-part="t.bar.title"] [data-kit="app-bar-title-button"]');
    expect(b?.hasAttribute('aria-haspopup')).toBe(false);
    b?.click();
    expect(tapped).toBe(1);
  });
});
