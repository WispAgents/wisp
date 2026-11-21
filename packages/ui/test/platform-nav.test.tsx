/// <reference types="vite/client" />
// ADR-0065 build step 3: the frame and navigation's phone variants (design/mobile.md §A.1, §A.7). Each
// component draws its platform's structure under `data-platform` (here a PlatformLookProvider), and the
// browser's render is byte for byte what it was before the variants landed (test/goldens/platform-browser.json).
import { describe, expect, it, vi } from 'vitest';
import goldenRaw from './goldens/platform-browser.json?raw';
import type { JSX } from 'solid-js';
import { mount, tick } from './helpers.tsx';
import { scenes } from './platform-scenes.tsx';
import { Fab, Frame, NavigationBar, PlatformLookProvider, Text, Toolbar, TopAppBar, backTitle, englishT, watchGiveWay, type Look, type NavigationItem } from '../src/index.ts';
import { FAB_SLOP, FAB_START, barCollapsed, fabOnScroll, type FabScroll } from '../src/pure.ts';
import { createSignal, Show } from 'solid-js';

const GOLDEN = JSON.parse(goldenRaw) as Record<string, string>;
const ITEMS: readonly NavigationItem[] = [
  { id: 'tasks', label: 'Tasks', icon: 'feed', iconSelected: 'feed-fill', badge: 3 },
  { id: 'crew', label: 'Crew', icon: 'crew', iconSelected: 'crew-fill' },
  { id: 'stuff', label: 'Stuff', icon: 'stuff' },
  { id: 'computers', label: 'Computers', icon: 'laptop' },
];
const noop = (): void => {};
const nav = (): JSX.Element => <NavigationBar aria-label="Destinations" items={ITEMS} selected="tasks" onSelect={noop} />;
const as = (look: Look, view: () => JSX.Element): HTMLElement => mount(() => <PlatformLookProvider look={() => look}>{view()}</PlatformLookProvider>);
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const qa = (sel: string): HTMLElement[] => [...document.querySelectorAll<HTMLElement>(sel)];

describe('the browser look is unchanged', () => {
  it('a_browser_render_of_frame_and_toolbar_is_byte_identical_to_before', () => {
    // the phone-only props passed too: a NavigationBar in `nav` is never drawn in the browser
    for (const [name, view] of scenes((kind) => (kind === 'frame' ? { nav: nav() } : {}))) {
      const host = mount(view);
      expect(host.innerHTML, name).toBe(GOLDEN[name]);
      host.remove();
    }
  });

  it('a_browser_fab_is_v1s_round_button_byte_identical_to_before', () => {
    const one = mount(() => <Fab icon="plus" label="New task" onClick={noop} />);
    expect(one.innerHTML).toBe(GOLDEN['fab']);
    one.remove();
    const away = mount(() => <Fab icon="plus" label="New task" giveWay small giveWayTo="[data-x]" onClick={noop} />);
    expect(away.innerHTML).toBe(GOLDEN['fab-give-way']);
  });

  it('a_browser_draws_no_navigation_bar_and_a_plain_top_bar', () => {
    const host = mount(() => (
      <>
        {nav()}
        <TopAppBar kind="root" title="Tasks" back={{ label: 'Tasks', onBack: noop }} />
      </>
    ));
    expect(host.querySelector('[data-kit="nav-bar"]')).toBeNull();
    expect(host.querySelector('[data-kit="app-bar"]')).toBeNull();
    expect(host.querySelector('header[data-kit="inline"][data-part="ui.app-bar"]')).not.toBeNull();
    expect(host.querySelector('[data-part="ui.app-bar.back"]')?.getAttribute('aria-label')).toBe('Tasks');
  });

  it('the_browser_stylesheet_carries_no_phone_variant', () => {
    const sheets = import.meta.glob<string>('../src/css/*.css', { query: '?raw', import: 'default', eager: true });
    for (const [path, raw] of Object.entries(sheets)) {
      const css = raw.replace(/\/\*[^]*?\*\//g, '');
      expect(css, path).not.toMatch(/data-platform|'nav-bar'|'nav-item'|'app-bar|'fab-label'|'frame-nav'/);
      expect(css, path).not.toMatch(/platform\//);
    }
  });
});

describe('NavigationBar', () => {
  it('android_draws_the_m3_bar_with_the_active_pill_on_the_selected_destination', () => {
    as('android', nav);
    const bar = q('nav[data-kit="nav-bar"]');
    expect(bar?.getAttribute('aria-label')).toBe('Destinations');
    const items = qa('[data-kit="nav-item"]');
    expect(items.map((i) => i.dataset['item'])).toEqual(['tasks', 'crew', 'stuff', 'computers']);
    expect(items.map((i) => i.getAttribute('aria-current'))).toEqual(['page', null, null, null]);
    expect(items.every((i) => i.querySelector('[data-kit="nav-pill"] > [data-kit="icon"]') && i.querySelector('[data-kit="nav-label"][data-text="control"]'))).toBe(true);
    expect(items[0]?.querySelector('[data-kit="nav-badge"]')?.textContent).toBe('3');
    expect(items[1]?.querySelector('[data-kit="nav-badge"]')).toBeNull();
  });

  it('ios_fills_the_selected_tabs_glyph_and_only_its', () => {
    as('ios', nav);
    const icons = qa('[data-kit="nav-item"] [data-kit="icon"]').map((i) => i.dataset['icon']);
    expect(icons).toEqual(['feed-fill', 'crew', 'stuff', 'laptop']);
  });

  it('selecting_a_destination_reports_it_and_marks_it_current_in_the_same_frame', async () => {
    const [sel, setSel] = createSignal('tasks');
    const picked: string[] = [];
    as('android', () => <NavigationBar aria-label="D" items={ITEMS} selected={sel()} onSelect={(id) => { picked.push(id); setSel(id); }} />);
    q('[data-item="crew"]')?.click();
    expect(picked).toEqual(['crew']);
    expect(q('[data-item="crew"]')?.getAttribute('aria-current')).toBe('page');
    expect(q('[data-item="crew"] [data-kit="icon"]')?.dataset['icon']).toBe('crew-fill');
    expect(q('[data-item="tasks"]')?.hasAttribute('data-selected')).toBe(false);
  });

  it('a_badge_over_99_reads_99_plus_and_has_its_accessible_text', () => {
    as('ios', () => <NavigationBar aria-label="D" items={[{ id: 'a', label: 'Tasks', icon: 'feed', badge: 140 }]} selected="a" onSelect={noop} badgeLabel={(n) => `${n} need you`} />);
    const b = q('[data-kit="nav-badge"]');
    expect(b?.textContent).toBe('99+');
    expect(b?.getAttribute('aria-label')).toBe('140 need you');
  });
});

describe('Frame on a phone', () => {
  for (const look of ['android', 'ios'] as const) {
    it(`${look}_draws_nav_under_the_panes`, () => {
      as(look, () => <Frame layout="stack" page="list" top={<Text>top</Text>} list={<Text>list</Text>} main={null} nav={nav()} />);
      const frame = q('[data-kit="frame"]');
      const kids = [...(frame?.children ?? [])].map((c) => c.getAttribute('data-kit'));
      expect(kids).toEqual(['frame-top', 'frame-body', 'frame-nav']);
      expect(q('[data-kit="frame-nav"] > nav[data-kit="nav-bar"]')).not.toBeNull();
    });
  }
  it('a_phone_frame_without_nav_draws_no_nav_box', () => {
    as('android', () => <Frame layout="stack" page="list" top={null} list={null} main={null} />);
    expect(q('[data-kit="frame-nav"]')).toBeNull();
  });
});

describe('Fab', () => {
  it('android_draws_the_extended_fab_with_its_label_and_a_small_one_without', () => {
    as('android', () => (
      <>
        <Fab icon="plus" label="New task" onClick={noop} />
        <Fab icon="plus" label="New job" small onClick={noop} part="t.small" />
      </>
    ));
    const [ext, small] = qa('[data-kit="fab"]');
    expect(ext?.hasAttribute('data-extended')).toBe(true);
    expect(ext?.getAttribute('aria-label')).toBe('New task');
    expect(ext?.querySelector('[data-kit="fab-label"][data-text="control"]')?.textContent).toBe('New task');
    expect(small?.hasAttribute('data-extended')).toBe(false);
    expect(small?.querySelector('[data-kit="fab-label"]')).toBeNull();
  });

  it('android_fab_shrinks_on_scroll_down_and_grows_back_on_scroll_up', async () => {
    as('android', () => <Frame layout="stack" page="list" top={null} list={<><div data-testid="scroller" /><Fab icon="plus" label="New task" onClick={noop} /></>} main={null} />);
    const scroller = q('[data-testid="scroller"]');
    if (!scroller) throw new Error('no scroller');
    Object.defineProperty(scroller, 'scrollHeight', { value: 2000 });
    Object.defineProperty(scroller, 'clientHeight', { value: 500 });
    const to = (y: number): void => {
      scroller.scrollTop = y;
      scroller.dispatchEvent(new Event('scroll'));
    };
    const fab = (): HTMLElement | null => q('[data-kit="fab"]');
    to(4);
    expect(fab()?.hasAttribute('data-extended')).toBe(true); // within the slop
    to(120);
    expect(fab()?.hasAttribute('data-extended')).toBe(false);
    to(300);
    to(280);
    expect(fab()?.hasAttribute('data-extended')).toBe(true);
    to(600);
    expect(fab()?.hasAttribute('data-extended')).toBe(false);
    to(0);
    expect(fab()?.hasAttribute('data-extended')).toBe(true);
    await tick();
  });

  it('a_fab_given_way_leaves_the_tab_order_and_the_reader', () => {
    as('android', () => <Fab icon="plus" label="New task" giveWay onClick={noop} />);
    const f = q('[data-kit="fab"]');
    expect(f?.hasAttribute('data-give-way')).toBe(true);
    expect(f?.getAttribute('aria-hidden')).toBe('true');
    expect(f?.tabIndex).toBe(-1);
  });

  it('ios_draws_no_fab_and_the_frames_top_bar_gets_the_compose_button', () => {
    const fn = vi.fn();
    as('ios', () => <Frame layout="stack" page="list" top={<Text>top</Text>} list={<Fab icon="plus" label="New task" onClick={fn} />} main={null} />);
    expect(q('[data-kit="fab"]')).toBeNull();
    const compose = qa('[data-kit="frame-top"] [data-kit="icon-button"]');
    expect(compose.length).toBe(1);
    expect(compose[0]?.getAttribute('aria-label')).toBe('New task');
    expect(compose[0]?.querySelector('[data-kit="icon"]')?.getAttribute('data-icon')).toBe('compose');
    compose[0]?.click();
    expect(fn).toHaveBeenCalledOnce();
  });

  it('ios_a_root_top_app_bar_draws_the_compose_button_once_at_its_end', () => {
    as('ios', () => <Frame layout="stack" page="list" top={<TopAppBar kind="root" title="Tasks" />} list={<Fab icon="plus" label="New task" onClick={noop} />} main={null} />);
    const buttons = qa('[aria-label="New task"]');
    expect(buttons.length).toBe(1);
    expect(buttons[0]?.closest('[data-kit="app-bar-actions"]')).not.toBeNull();
  });

  it('ios_compose_shows_on_tasks_only_not_on_my_stuffs_bar_r298', async () => {
    // the Tasks list stays mounted (hidden) under My Stuff with its Fab: My Stuff's root bar never draws New task
    const [page, setPage] = createSignal<'list' | 'main'>('list');
    as('ios', () => (
      <Frame layout="stack" page={page()} top={<TopAppBar kind="root" title="Tasks" part="t.bar" />} list={<Fab icon="plus" label="New task" onClick={noop} />} main={<TopAppBar kind="root" title="My Stuff" part="s.bar" />} />
    ));
    expect(qa('[data-part="t.bar"] [aria-label="New task"]').length).toBe(1);
    setPage('main');
    expect(qa('[data-part="s.bar"] [aria-label="New task"]').length).toBe(0);
    expect(qa('[data-kit="frame-pane"][data-pane="main"] [aria-label="New task"]').length).toBe(0);
    // with no bar of its own on the shown page, the frame's top bar doesn't draw a hidden pane's action either
    as('ios', () => <Frame layout="stack" page="main" top={null} list={<Fab icon="plus" label="Other" onClick={noop} />} main={null} />);
    expect(qa('[aria-label="Other"]').length).toBe(0);
  });

  it('ios_the_compose_button_leaves_with_its_fab', async () => {
    const [on, setOn] = createSignal(true);
    as('ios', () => <Frame layout="stack" page="list" top={null} list={<Show when={on()}><Fab icon="plus" label="New task" onClick={noop} /></Show>} main={null} />);
    expect(qa('[aria-label="New task"]').length).toBe(1);
    setOn(false);
    expect(qa('[aria-label="New task"]').length).toBe(0);
  });
});

describe('give way (v1 fab.js)', () => {
  it('the_fab_gives_way_while_a_rows_choices_are_on_screen_and_comes_back', () => {
    const observed = new Set<Element>();
    let fire: ((entries: Array<{ target: Element; isIntersecting: boolean }>) => void) | undefined;
    class FakeIO {
      constructor(cb: (entries: Array<{ target: Element; isIntersecting: boolean }>) => void) {
        fire = cb;
      }
      observe(el: Element): void {
        observed.add(el);
      }
      unobserve(el: Element): void {
        observed.delete(el);
      }
      disconnect(): void {
        observed.clear();
      }
    }
    vi.stubGlobal('IntersectionObserver', FakeIO);
    const root = document.createElement('div');
    const choice = document.createElement('div');
    choice.setAttribute('data-choices', '');
    root.append(choice);
    const seen: boolean[] = [];
    const stop = watchGiveWay(root, '[data-choices]', (a) => seen.push(a));
    expect(observed.has(choice)).toBe(true);
    fire?.([{ target: choice, isIntersecting: true }]);
    fire?.([{ target: choice, isIntersecting: true }]);
    fire?.([{ target: choice, isIntersecting: false }]);
    expect(seen).toEqual([true, false]);
    stop();
    vi.unstubAllGlobals();
  });
});

describe('TopAppBar', () => {
  it('android_page_is_the_small_bar_its_title_beside_back', () => {
    as('android', () => <TopAppBar kind="page" title="Fix the flaky test" back={{ label: 'Tasks', onBack: noop }} />);
    const bar = q('header[data-kit="app-bar"]');
    expect(bar?.dataset['kind']).toBe('page');
    // no medium bar: its inline title is hidden until the page scrolls, so the title would sit under the arrow
    expect(bar?.hasAttribute('data-large')).toBe(false);
    expect(q('[data-kit="app-bar-large"]')).toBeNull();
    expect(q('[data-kit="app-bar-row"] > [data-kit="icon-button"] + h1[data-kit="app-bar-title"]')?.textContent).toBe('Fix the flaky test');
    // Android's Back is an icon button named after where it goes
    expect(q('[data-kit="app-bar-row"] > [data-kit="icon-button"]')?.getAttribute('aria-label')).toBe('Tasks');
  });

  it('android_root_is_the_small_bar_and_ios_root_has_the_large_title', () => {
    as('android', () => <TopAppBar kind="root" title="Tasks" />);
    expect(q('[data-kit="app-bar"]')?.hasAttribute('data-large')).toBe(false);
    expect(q('[data-kit="app-bar-large"]')).toBeNull();
    document.body.innerHTML = '';
    as('ios', () => <TopAppBar kind="root" title="Tasks" />);
    expect(q('[data-kit="app-bar"]')?.hasAttribute('data-large')).toBe(true);
    expect(q('[data-kit="app-bar-large-title"]')?.getAttribute('aria-hidden')).toBe('true');
  });

  it('ios_page_has_the_inline_title_and_back_carries_the_previous_title', () => {
    const back = vi.fn();
    as('ios', () => <TopAppBar kind="page" title="Settings" back={{ label: 'Tasks', onBack: back }} />);
    expect(q('[data-kit="app-bar-large"]')).toBeNull();
    const b = q('[data-kit="app-bar-row"] > [data-kit="button"]');
    expect(b?.textContent).toBe('Tasks');
    b?.click();
    expect(back).toHaveBeenCalledOnce();
  });

  it('a_long_back_title_shortens_to_back_on_ios', () => {
    expect(backTitle('Tasks', englishT)).toBe('Tasks');
    expect(backTitle('Computers and cloud', englishT)).toBe('Back');
  });

  it('the_large_title_folds_on_scroll_and_comes_back_at_the_top', () => {
    as('ios', () => <Frame layout="stack" page="list" top={<TopAppBar kind="root" title="Tasks" />} list={<div data-testid="s" />} main={null} />);
    const s = q('[data-testid="s"]');
    if (!s) throw new Error('no scroller');
    Object.defineProperty(s, 'scrollHeight', { value: 2000 });
    Object.defineProperty(s, 'clientHeight', { value: 500 });
    const to = (y: number): void => {
      s.scrollTop = y;
      s.dispatchEvent(new Event('scroll'));
    };
    to(40);
    expect(q('[data-kit="app-bar"]')?.hasAttribute('data-collapsed')).toBe(true);
    to(0);
    expect(q('[data-kit="app-bar"]')?.hasAttribute('data-collapsed')).toBe(false);
  });

  it('the_title_button_opens_the_filter', () => {
    const go = vi.fn();
    as('android', () => <TopAppBar kind="root" title="All" onTitle={go} />);
    q('[data-kit="app-bar-title"] > [data-kit="app-bar-title-button"]')?.click();
    expect(go).toHaveBeenCalledOnce();
  });
});

describe('Toolbar', () => {
  it('more_is_the_vertical_dots_on_android_and_the_horizontal_ones_elsewhere', () => {
    const items = [{ id: 'a', label: 'A', icon: 'done', priority: 1, onSelect: noop }] as const;
    as('android', () => <Toolbar aria-label="T" items={items} />);
    expect(q('[data-measure="more"] [data-kit="icon"]')?.dataset['icon']).toBe('dots-v');
    document.body.innerHTML = '';
    as('ios', () => <Toolbar aria-label="T" items={items} />);
    expect(q('[data-measure="more"] [data-kit="icon"]')?.dataset['icon']).toBe('dots');
  });
});

describe('scroll rules (pure)', () => {
  const walk = (ys: readonly number[], from: FabScroll = FAB_START): FabScroll => ys.reduce(fabOnScroll, from);

  it('the_fab_is_extended_at_the_top_whatever_came_before', () => {
    let seed = 7;
    const rand = (): number => (seed = (seed * 16807) % 2147483647) / 2147483647;
    for (let run = 0; run < 200; run++) {
      const ys = Array.from({ length: 30 }, () => Math.round(rand() * 3000));
      expect(walk([...ys, 0]).extended).toBe(true);
    }
  });

  it('jitter_within_the_slop_never_flips_the_fab', () => {
    const down = walk([0, 100, 200]);
    expect(down.extended).toBe(false);
    for (let i = 0; i < 50; i++) expect(walk([200 - (i % FAB_SLOP), 200]).extended).toBe(false);
    expect(fabOnScroll(down, 200 - FAB_SLOP - 1).extended).toBe(true);
  });

  it('a_steady_scroll_down_past_the_slop_shrinks_it_and_any_scroll_up_past_it_grows_it', () => {
    let seed = 3;
    const rand = (): number => (seed = (seed * 48271) % 2147483647) / 2147483647;
    for (let run = 0; run < 200; run++) {
      const start = walk([0, Math.round(rand() * 500) + 1]);
      const y = start.anchor + FAB_SLOP + 1 + Math.round(rand() * 400);
      const down = walk([y, y + FAB_SLOP + 1], start);
      expect(down.extended).toBe(false);
      expect(fabOnScroll(down, down.anchor - FAB_SLOP - 1).extended).toBe(true);
    }
  });

  it('the_bar_folds_past_half_the_large_title_and_unfolds_under_a_quarter', () => {
    expect(barCollapsed(false, 25, 52)).toBe(false);
    expect(barCollapsed(false, 27, 52)).toBe(true);
    expect(barCollapsed(true, 14, 52)).toBe(true);
    expect(barCollapsed(true, 13, 52)).toBe(false);
    expect(barCollapsed(false, 1, 0)).toBe(true);
    expect(barCollapsed(true, 0, 0)).toBe(false);
  });
});
