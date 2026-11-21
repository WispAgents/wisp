// ADR-0065 step 13 (design/mobile.md §A.2): the kit's parts of the phone apps' task page, New task and Search. The
// TopAppBar's page menu (⋮ / ⋯), its compose bar (Android's full-screen dialog bar: × and a text Send; iOS's sheet bar:
// Cancel and Send), its search (Android's icon, iOS's field under the large title), the palette as the platform's search
// view (Back / Cancel), and the Frame's New-task-over-the-list flag. The browser draws none of it.
import { describe, expect, it } from 'vitest';
import type { JSX } from 'solid-js';
import { mount } from './helpers.tsx';
import { Frame, PlatformLookProvider, TopAppBar, type Look } from '../src/index.ts';
import { PaletteSheet, type PaletteRow } from '../src/app/Palette.tsx';

const noop = (): void => {};
const as = (look: Look, view: () => JSX.Element): HTMLElement => mount(() => <PlatformLookProvider look={() => look}>{view()}</PlatformLookProvider>);
const MENU = [{ value: 'done', label: 'Mark done', icon: 'done' as const }];

describe('TopAppBar on a task page and on New task', () => {
  for (const [look, icon] of [
    ['android', 'dots-v'],
    ['ios', 'dots'],
  ] as const) {
    it(`a_page_bar_on_${look}_has_back_and_the_pages_menu_with_the_platforms_more_icon`, () => {
      const host = as(look, () => <TopAppBar kind="page" part="t.bar" title="Add rate limiting" back={{ label: 'Tasks', onBack: noop }} menu={{ label: 'More', items: MENU, onSelect: noop }} />);
      const bar = host.querySelector('[data-kit="app-bar"]');
      expect(bar?.getAttribute('data-kind')).toBe('page');
      expect(host.querySelector('[data-part="t.bar.back"]')).not.toBeNull();
      const more = host.querySelector('[aria-label="More"]');
      expect(more, 'the page’s menu').not.toBeNull();
      expect(more?.querySelector(`svg[data-icon="${icon}"]`), `${icon}: M3’s ⋮ / iOS’s ⋯`).not.toBeNull();
    });
  }

  it('a_compose_bar_on_android_is_the_full_screen_dialogs_close_title_and_text_send', () => {
    let sent = 0;
    const host = as('android', () => (
      <TopAppBar kind="compose" part="t.new" title="New task" back={{ label: 'Close', onBack: noop, close: 'Cancel' }} confirm={{ label: 'Send', onSelect: () => sent++ }} />
    ));
    const bar = host.querySelector('[data-kit="app-bar"]');
    expect(bar?.getAttribute('data-kind')).toBe('compose');
    expect(bar?.hasAttribute('data-large'), 'one line: no medium title').toBe(false);
    const out = host.querySelector('[data-part="t.new.back"]');
    expect(out?.getAttribute('aria-label')).toBe('Close');
    expect(out?.textContent, '× only').toBe('');
    const send = host.querySelector<HTMLButtonElement>('[data-kit="app-bar-confirm"] [data-part="t.new.confirm"]');
    expect(send?.textContent).toBe('Send');
    send?.click();
    expect(sent).toBe(1);
  });

  it('a_compose_bar_on_ios_is_the_sheets_cancel_title_and_send_and_send_waits_for_a_prompt', () => {
    const host = as('ios', () => (
      <TopAppBar kind="compose" part="t.new" title="New task" back={{ label: 'Close', onBack: noop, close: 'Cancel' }} confirm={{ label: 'Send', onSelect: noop, disabled: true }} />
    ));
    expect(host.querySelector('[data-part="t.new.back"]')?.textContent).toBe('Cancel');
    expect(host.querySelector<HTMLButtonElement>('[data-part="t.new.confirm"]')?.disabled).toBe(true);
    expect(host.querySelector('[data-kit="app-bar"]')?.hasAttribute('data-large')).toBe(false);
  });

});

describe('TopAppBar search, and the browser', () => {
  it('search_on_android_is_the_bars_icon_and_on_ios_the_field_under_the_large_title', () => {
    let opened = 0;
    const search = { label: 'Search', onOpen: () => opened++ };
    const a = as('android', () => <TopAppBar kind="root" part="t.list" title="All" search={search} />);
    expect(a.querySelector('[data-kit="app-bar-search-icon"] [aria-label="Search"]')).not.toBeNull();
    expect(a.querySelector('[data-kit="app-bar-search"]'), 'no field on Android').toBeNull();
    a.remove();
    const i = as('ios', () => <TopAppBar kind="root" part="t.list" title="All" search={search} />);
    const field = i.querySelector<HTMLButtonElement>('[data-kit="app-bar-search"] [data-part="t.list.search-field"]');
    expect(field?.textContent).toBe('Search');
    // under the large title, in the same header (it folds away with it)
    expect(i.querySelector('[data-kit="app-bar-large"] + [data-kit="app-bar-search"]')).not.toBeNull();
    field?.click();
    expect(opened).toBe(1);
  });

  it('a_browser_bar_draws_no_platform_parts_for_search_menu_or_send', () => {
    const host = mount(() => <TopAppBar kind="page" part="t.bar" title="T" search={{ label: 'Search', onOpen: noop }} menu={{ label: 'More', items: MENU, onSelect: noop }} confirm={{ label: 'Send', onSelect: noop }} />);
    expect(host.querySelector('[data-kit="app-bar"]')).toBeNull();
    expect(host.querySelector('[data-kit="app-bar-search"], [data-kit="app-bar-confirm"], [data-kit="app-bar-search-icon"]')).toBeNull();
    // the plain bar still offers them as plain controls (no caller passes them in the browser today)
    expect(host.querySelector('[data-part="t.bar.search"]')).not.toBeNull();
  });
});

const ROWS: PaletteRow[] = [{ id: 't1', head: 'Tasks', label: [['Add rate limiting', false]], sub: 'Finished', dot: 'pc-violet' }];
const sheet = (close?: { back: string; cancel: string; onClose: () => void }): (() => JSX.Element) => () => (
  <PaletteSheet
    value=""
    placeholder="Search"
    label="Search"
    rows={ROWS}
    selected={0}
    empty="None"
    foot={{ navigate: 'navigate', open: 'open', close: 'close' }}
    onInput={noop}
    onKeyDown={noop}
    onPick={noop}
    onHover={noop}
    {...(close && { close })}
  />
);

describe('the palette as the phone apps’ search view', () => {
  it('android_search_view_has_back_at_the_fields_start_in_place_of_the_glyph', () => {
    let closed = 0;
    const host = as('android', sheet({ back: 'Back', cancel: 'Cancel', onClose: () => closed++ }));
    const back = host.querySelector<HTMLButtonElement>('[data-kit="pal-input"] > [data-part="shell.palette.back"]');
    expect(back?.getAttribute('aria-label')).toBe('Back');
    expect(host.querySelector('[data-kit="pal-input"] > [data-kit="icon"]'), 'no search glyph').toBeNull();
    expect(host.querySelector('[data-part="shell.palette.cancel"]')).toBeNull();
    back?.click();
    expect(closed).toBe(1);
  });

  it('ios_search_field_has_cancel_at_its_end', () => {
    const host = as('ios', sheet({ back: 'Back', cancel: 'Cancel', onClose: noop }));
    const input = host.querySelector('[data-kit="pal-input"]');
    expect(input?.lastElementChild?.getAttribute('data-part')).toBe('shell.palette.cancel');
    expect(input?.lastElementChild?.textContent).toBe('Cancel');
    expect(host.querySelector('[data-part="shell.palette.back"]')).toBeNull();
  });

  it('the_browsers_palette_is_byte_identical_with_or_without_close', () => {
    const plain = mount(sheet());
    const html = plain.innerHTML;
    plain.remove();
    const withClose = mount(sheet({ back: 'Back', cancel: 'Cancel', onClose: noop }));
    expect(withClose.innerHTML).toBe(html);
  });
});

describe('Frame: New task over the list', () => {
  const frame = (behind: boolean | undefined): (() => JSX.Element) => () => (
    <Frame layout="stack" page="main" top={<span />} list={<span />} main={<span />} {...(behind !== undefined && { behind })} />
  );
  it('the_frame_says_new_task_is_over_the_list_only_when_told', () => {
    const on = as('ios', frame(true));
    expect(on.querySelector('[data-kit="frame"]')?.hasAttribute('data-behind')).toBe(true);
    on.remove();
    const off = as('ios', frame(undefined));
    expect(off.querySelector('[data-kit="frame"]')?.hasAttribute('data-behind')).toBe(false);
  });
});
