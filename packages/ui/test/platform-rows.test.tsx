/// <reference types="vite/client" />
// ADR-0065 build step 4: the rows' and surfaces' phone variants (design/mobile.md §A.2, §A.4, §A.7). Each
// component draws its platform's structure under `data-platform` (here a PlatformLookProvider), and the
// browser's render is byte for byte what it was before the variants landed (test/goldens/platform-browser-rows.json).
import { afterEach, describe, expect, it, vi } from 'vitest';
import goldenRaw from './goldens/platform-browser-rows.json?raw';
import type { JSX } from 'solid-js';
import { mount } from './helpers.tsx';
import { rowScenes } from './platform-scenes-rows.tsx';
import {
  Card,
  ListRow,
  PlatformLookProvider,
  PullRefresh,
  PULL_TICKS,
  SWIPE_ACTION_W,
  SWIPE_COMMIT,
  SWIPE_EDGE,
  SWIPE_FULL,
  SWIPE_GIVE,
  Section,
  SettingsGroup,
  SettingsRow,
  Swipe,
  Text,
  VirtualList,
  pullAmount,
  rowLines,
  swipeArmedFor,
  swipeOffset,
  swipeProgress,
  swipeRelease,
  swipeStarts,
  type Look,
  type SwipeDir,
} from '../src/index.ts';

const GOLDEN = JSON.parse(goldenRaw) as Record<string, string>;
const noop = (): void => {};
const as = (look: Look, view: () => JSX.Element): HTMLElement => mount(() => <PlatformLookProvider look={() => look}>{view()}</PlatformLookProvider>);
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const qa = (sel: string): HTMLElement[] => [...document.querySelectorAll<HTMLElement>(sel)];
const PHONE_EXTRA = (kind: string): Record<string, unknown> =>
  kind === 'row' ? { push: true } : kind === 'card' ? { variant: 'filled' } : kind === 'virtual' ? { pull: { progress: 0.6, refreshing: true, label: 'Reconnecting' } } : {};

describe('the browser look is unchanged', () => {
  it('a_browser_render_of_every_row_and_surface_is_byte_identical_to_before', () => {
    // the phone-only props passed too (push, variant, pull): the browser ignores every one
    for (const [name, view] of rowScenes(PHONE_EXTRA)) {
      const host = mount(view);
      expect(host.innerHTML, name).toBe(GOLDEN[name]);
      host.remove();
    }
  });

  it('the_browser_stylesheet_carries_none_of_the_row_variants', () => {
    const sheets = import.meta.glob<string>('../src/css/*.css', { query: '?raw', import: 'default', eager: true });
    for (const [path, raw] of Object.entries(sheets)) {
      const css = raw.replace(/\/\*[^]*?\*\//g, '');
      expect(css, path).not.toMatch(/data-platform|'list-row-chevron'|'swipe-action'|'pull-refresh'|'pull-disc'|'pull-tick'|'section-footer'|'settings-group-footer'|data-row-lines|--k-sw-p|--k-sw-w/);
    }
  });

  it('a_browser_pull_draws_nothing_at_all', () => {
    const host = mount(() => <PullRefresh progress={1} refreshing label="Reconnecting" />);
    expect(host.innerHTML).toBe('');
  });
});

describe('ListRow', () => {
  it('a_row_counts_its_lines_from_its_sub_and_foot', () => {
    expect(rowLines({})).toBe('1');
    expect(rowLines({ sub: 'Studio' })).toBe('2');
    expect(rowLines({ sub: 'Studio', foot: 'Editing' })).toBe('3');
    expect(rowLines({ foot: 'Editing' })).toBe('3');
  });

  for (const look of ['android', 'ios'] as const) {
    it(`${look}_marks_one_two_and_three_line_rows_and_a_leading_element`, () => {
      as(look, () => (
        <>
          <ListRow title="One" />
          <ListRow title="Two" sub={<Text>sub</Text>} lead={<Text>A</Text>} />
          <ListRow title="Three" sub={<Text>sub</Text>} foot={<Text>foot</Text>} />
          <ListRow layout="feed" title="Feed" sub={<Text>sub</Text>} />
        </>
      ));
      const rows = qa('[data-kit="list-row"]');
      expect(rows.map((r) => r.dataset['rowLines'])).toEqual(['1', '2', '3', '2']);
      expect(rows.map((r) => r.hasAttribute('data-lead'))).toEqual([false, true, false, false]);
    });
  }

  it('ios_draws_the_chevron_only_where_opening_pushes_a_page', () => {
    as('ios', () => (
      <>
        <ListRow title="Notifications" push onOpen={noop} id="pushes" />
        <ListRow title="Task" onOpen={noop} id="opens" />
        <ListRow title="Static" push id="static" />
        <ListRow title="Link" push href="/settings/keys" id="link" />
      </>
    ));
    expect(q('#pushes [data-kit="list-row-chevron"] [data-icon="chevron"]')).not.toBeNull();
    expect(q('#pushes [data-kit="list-row-chevron"]')?.getAttribute('aria-hidden')).toBe('true');
    expect(q('#opens [data-kit="list-row-chevron"]')).toBeNull();
    expect(q('#static [data-kit="list-row-chevron"]')).toBeNull();
    expect(q('#link [data-kit="list-row-chevron"]')).not.toBeNull();
  });

  it('android_draws_no_chevron_m3_list_items_have_none', () => {
    as('android', () => <ListRow title="Notifications" push onOpen={noop} />);
    expect(q('[data-kit="list-row-chevron"]')).toBeNull();
  });

  it('a_row_title_stays_control_text_on_both_phones', () => {
    for (const look of ['android', 'ios'] as const) {
      const host = as(look, () => <ListRow title="A very long task title that must truncate on one line" sub={<Text>sub</Text>} />);
      expect(host.querySelector('[data-kit="list-row-title"]')?.getAttribute('data-text')).toBe('control');
      host.remove();
    }
  });
});

describe('Section and SettingsGroup', () => {
  it('ios_moves_the_help_text_under_the_box_as_the_footer', () => {
    as('ios', () => (
      <>
        <Section title="Recent" description="Last seven days">
          <Text>body</Text>
        </Section>
        <SettingsGroup title="Notifications" description="What reaches this phone.">
          <SettingsRow title="Finished" />
        </SettingsGroup>
      </>
    ));
    expect(q('[data-kit="section-description"]')).toBeNull();
    expect(q('[data-kit="settings-group-description"]')).toBeNull();
    const section = q('[data-kit="section"]');
    expect([...(section?.children ?? [])].map((c) => c.getAttribute('data-kit'))).toEqual(['section-head', 'section-body', 'section-footer']);
    expect(q('[data-kit="section-footer"]')?.textContent).toBe('Last seven days');
    const group = q('[data-kit="settings-group"]');
    expect([...(group?.children ?? [])].map((c) => c.getAttribute('data-kit'))).toEqual(['settings-group-head', 'settings-group-body', 'settings-group-footer']);
    expect(q('[data-kit="settings-group-footer"]')?.dataset['part']).toBe('settings.group.footer');
  });

  it('android_keeps_the_help_text_under_the_subheader_and_draws_no_footer', () => {
    as('android', () => (
      <SettingsGroup title="Notifications" description="What reaches this phone.">
        <SettingsRow title="Finished" />
      </SettingsGroup>
    ));
    expect(q('[data-kit="settings-group-description"]')?.textContent).toBe('What reaches this phone.');
    expect(q('[data-kit="settings-group-footer"]')).toBeNull();
  });

  it('a_group_without_help_text_has_no_footer_box', () => {
    as('ios', () => <SettingsGroup title="Keys"><SettingsRow title="Make a key" /></SettingsGroup>);
    expect(q('[data-kit="settings-group-footer"]')).toBeNull();
  });
});

describe('Card', () => {
  it('a_phone_card_is_outlined_unless_asked_filled', () => {
    as('android', () => (
      <>
        <Card id="a">x</Card>
        <Card id="b" variant="filled">y</Card>
      </>
    ));
    expect(q('#a')?.dataset['variant']).toBe('outlined');
    expect(q('#b')?.dataset['variant']).toBe('filled');
  });
});

describe('Swipe: the math', () => {
  const W = 400;
  it('android_commits_past_a_third_and_ios_runs_past_half', () => {
    expect(swipeRelease('band', W * 0.35, W, true, 0)).toBe('commit');
    expect(swipeRelease('band', W * 0.3, W, true, 0)).toBe('close');
    expect(swipeRelease('reveal', W * 0.35, W, true, 0)).toBe('open');
    expect(swipeRelease('reveal', W * 0.51, W, true, 0)).toBe('commit');
  });

  it('ios_rests_open_past_half_a_button_and_springs_back_short_of_it', () => {
    expect(swipeRelease('reveal', SWIPE_ACTION_W / 2, W, true, 0)).toBe('open');
    expect(swipeRelease('reveal', SWIPE_ACTION_W / 2 - 1, W, true, 0)).toBe('close');
    expect(swipeRelease('reveal', -SWIPE_ACTION_W, W, true, 0)).toBe('open');
    // a flick out opens from a short pull; a flick back closes from a long one
    expect(swipeRelease('reveal', 20, W, true, 0.8)).toBe('open');
    expect(swipeRelease('reveal', 150, W, true, -0.8)).toBe('close');
  });

  it('a_side_with_no_action_only_gives_a_little_and_never_runs_on_either_phone', () => {
    for (const style of ['band', 'reveal'] as const) {
      expect(swipeRelease(style, W, W, false, 2)).toBe('close');
      expect(swipeArmedFor(style, W, W, false)).toBe(false);
      expect(swipeProgress(style, W, W, false)).toBe(0);
    }
    expect(Math.abs(swipeOffset(300, false))).toBe(SWIPE_GIVE);
  });

  it('the_band_warms_to_1_on_the_line_and_the_tick_comes_just_past_it', () => {
    expect(swipeProgress('band', W * SWIPE_COMMIT, W, true)).toBeCloseTo(1);
    expect(swipeProgress('reveal', W * SWIPE_FULL, W, true)).toBeCloseTo(1);
    expect(swipeProgress('reveal', W * 0.25, W, true)).toBeCloseTo(0.5);
    expect(swipeArmedFor('band', W * SWIPE_COMMIT + 1, W, true)).toBe(true);
    expect(swipeArmedFor('reveal', W * SWIPE_COMMIT + 1, W, true)).toBe(false);
    expect(swipeArmedFor('reveal', W * SWIPE_FULL + 1, W, true)).toBe(true);
  });

  it('the_24px_edge_zone_stays_the_systems_back', () => {
    expect(SWIPE_EDGE).toBe(24);
    expect(swipeStarts(SWIPE_EDGE, 412)).toBe(false);
    expect(swipeStarts(412 - SWIPE_EDGE, 412)).toBe(false);
    expect(swipeStarts(SWIPE_EDGE + 1, 412)).toBe(true);
  });

  it('the_math_uses_the_platform_specs_numbers', () => {
    // the same numbers as @wisp/tokens' --swipe-action-w / --swipe-line (pinned there by test/platform.test.ts)
    expect(SWIPE_ACTION_W).toBe(74);
    expect(SWIPE_FULL).toBe(0.5);
    // v1's 0.34 is "a third of the row" (android's --swipe-line 0.33)
    expect(Math.abs(SWIPE_COMMIT - 1 / 3)).toBeLessThan(0.01);
  });
});

describe('Swipe: the component', () => {
  afterEach(() => vi.restoreAllMocks());
  const W = 400;
  const rect = (): void => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({ width: W, height: 72, top: 0, left: 0, right: W, bottom: 72, x: 0, y: 0, toJSON: () => ({}) } as DOMRect);
  };
  const pe = (type: string, x: number, t: number, target?: Element): void => {
    const el = target ?? q('[data-kit="swipe"]');
    el?.dispatchEvent(new PointerEvent(type, { bubbles: true, clientX: x, clientY: 100, pointerType: 'touch', isPrimary: true, pointerId: 1, timeStamp: t } as PointerEventInit));
  };
  const drag = (from: number, to: number): void => {
    pe('pointerdown', from, 0);
    for (let i = 1; i <= 10; i++) pe('pointermove', from + ((to - from) * i) / 10, i * 50);
  };
  const flyOff = (): void => {
    const s = q('[data-kit="swipe"]');
    s?.dispatchEvent(Object.assign(new Event('transitionend', { bubbles: true }), { propertyName: 'transform' }));
  };
  const view = (swiped: SwipeDir[], armed: boolean[], running = false) => (): JSX.Element => (
    <Swipe right={running ? undefined : { label: 'Mark done', icon: 'done', tone: 'good' }} left={{ label: 'Save for later', icon: 'bookmark', tone: 'accent' }} onSwipe={(d) => swiped.push(d)} onArm={(a) => armed.push(a)}>
      <ListRow title="Row" onOpen={noop} />
    </Swipe>
  );

  it('android_band_names_the_action_warms_and_ticks_once_past_a_third', () => {
    rect();
    const swiped: SwipeDir[] = [];
    const armed: boolean[] = [];
    as('android', view(swiped, armed));
    drag(100, 100 + W * 0.2);
    const s = q('[data-kit="swipe"]');
    expect(q('[data-kit="swipe-band"] [data-kit="swipe-label"]')?.textContent).toBe('Mark done');
    expect(q('[data-kit="swipe-band"] [data-icon="done"]')).not.toBeNull();
    expect(Number(s?.style.getPropertyValue('--k-sw-p'))).toBeCloseTo(0.2 / SWIPE_COMMIT, 2);
    expect(armed).toEqual([]);
    drag(100, 100 + W * 0.4);
    expect(armed).toEqual([true]);
    expect(s?.hasAttribute('data-armed')).toBe(true);
    expect(q('[data-kit="swipe-action"]')).toBeNull();
  });

  it('ios_short_pull_rests_open_on_the_action_and_tapping_it_runs_it', () => {
    rect();
    const swiped: SwipeDir[] = [];
    as('ios', view(swiped, []));
    drag(300, 300 - W * 0.3);
    pe('pointerup', 300 - W * 0.3, 600);
    const s = q('[data-kit="swipe"]');
    expect(s?.dataset['phase']).toBe('open');
    expect(s?.style.getPropertyValue('--k-sw-x')).toBe(`${-SWIPE_ACTION_W}px`);
    expect(s?.style.getPropertyValue('--k-sw-w')).toBe(`${SWIPE_ACTION_W}px`);
    const action = q('button[data-kit="swipe-action"]');
    expect(action?.textContent).toBe('Save for later');
    expect(swiped).toEqual([]);
    pe('pointerdown', 380, 700, action ?? undefined);
    action?.click();
    expect(s?.dataset['phase']).toBe('gone');
    flyOff();
    expect(s?.dataset['phase']).toBe('gone'); // the height closes next frame, then it runs
  });

  it('ios_a_touch_on_the_open_row_closes_it_and_opens_nothing', () => {
    rect();
    const opened: number[] = [];
    as('ios', () => (
      <Swipe left={{ label: 'Save for later', icon: 'bookmark', tone: 'accent' }} onSwipe={noop}>
        <ListRow title="Row" onOpen={() => opened.push(1)} />
      </Swipe>
    ));
    drag(300, 300 - W * 0.3);
    pe('pointerup', 300 - W * 0.3, 600);
    const row = q('[data-kit="list-row-hit"]');
    pe('pointerdown', 100, 700, row ?? undefined);
    row?.click();
    expect(q('[data-kit="swipe"]')?.dataset['phase']).toBe('idle');
    expect(opened).toEqual([]);
  });

  it('ios_an_open_row_closes_when_another_row_or_the_page_is_touched', () => {
    rect();
    as('ios', () => (
      <>
        <Swipe id="a" left={{ label: 'Save for later', icon: 'bookmark', tone: 'accent' }} onSwipe={noop}>
          <ListRow title="A" />
        </Swipe>
        <ListRow id="b" title="B" />
      </>
    ));
    const a = q('#a') ?? undefined;
    pe('pointerdown', 300, 0, a);
    for (let i = 1; i <= 10; i++) pe('pointermove', 300 - (W * 0.3 * i) / 10, i * 50, a);
    pe('pointerup', 300 - W * 0.3, 600, a);
    expect(q('#a')?.dataset['phase']).toBe('open');
    pe('pointerdown', 100, 700, q('#b') ?? undefined);
    expect(q('#a')?.dataset['phase']).toBe('idle');
  });

  it('ios_full_swipe_past_half_the_row_ticks_and_runs_the_action_at_once', () => {
    rect();
    const armed: boolean[] = [];
    as('ios', view([], armed));
    drag(100, 100 + W * 0.45);
    expect(armed).toEqual([]);
    drag(100, 100 + W * 0.6);
    expect(armed).toEqual([true]);
    pe('pointerup', 100 + W * 0.6, 600);
    expect(q('[data-kit="swipe"]')?.dataset['phase']).toBe('gone');
  });

  for (const look of ['android', 'ios'] as const) {
    it(`${look}_a_running_row_only_gives_a_little_with_no_label`, () => {
      rect();
      const armed: boolean[] = [];
      as(look, view([], armed, true));
      drag(100, 100 + W * 0.8);
      const s = q('[data-kit="swipe"]');
      expect(Math.abs(Number.parseFloat(s?.style.getPropertyValue('--k-sw-x') ?? '0'))).toBeLessThanOrEqual(SWIPE_GIVE);
      expect(q('[data-kit="swipe-band"]')).toBeNull();
      expect(armed).toEqual([]);
      pe('pointerup', 100 + W * 0.8, 600);
      expect(s?.dataset['phase']).toBe('idle');
    });
  }

  it('a_touch_in_the_edge_zone_never_starts_a_row_swipe', () => {
    rect();
    as('ios', view([], []));
    drag(SWIPE_EDGE - 4, 300);
    expect(q('[data-kit="swipe"]')?.dataset['phase']).toBe('idle');
  });
});

describe('PullRefresh', () => {
  it('android_draws_the_m3_disc_and_ios_the_refresh_controls_ticks', () => {
    as('android', () => <PullRefresh progress={0.5} refreshing={false} label="Reconnecting" />);
    expect(q('[data-kit="pull-refresh"] [data-kit="pull-disc"] svg[data-kit="pull-arc"]')).not.toBeNull();
    expect(q('[data-kit="pull-refresh"]')?.style.getPropertyValue('--k-pull')).toBe('0.5');
    expect(q('[data-kit="pull-refresh"]')?.getAttribute('aria-hidden')).toBe('true');
    document.body.innerHTML = '';
    as('ios', () => <PullRefresh progress={0.5} refreshing={false} label="Reconnecting" />);
    expect(qa('[data-kit="pull-tick"]')).toHaveLength(PULL_TICKS);
    expect(q('[data-kit="pull-disc"]')).toBeNull();
  });

  it('refreshing_is_announced_and_shows_full_at_rest_it_draws_nothing', () => {
    as('ios', () => (
      <>
        <PullRefresh id="on" progress={0} refreshing label="Reconnecting" />
        <PullRefresh id="rest" progress={0} refreshing={false} label="Reconnecting" />
      </>
    ));
    const on = q('#on');
    expect(on?.getAttribute('role')).toBe('status');
    expect(on?.getAttribute('aria-label')).toBe('Reconnecting');
    expect(on?.style.getPropertyValue('--k-pull')).toBe('1');
    expect(q('#rest')).toBeNull();
  });

  it('the_pull_is_clamped_for_the_css', () => {
    expect(pullAmount(-1, false)).toBe('0');
    expect(pullAmount(0.12345, false)).toBe('0.123');
    expect(pullAmount(9, false)).toBe('1.5');
    expect(pullAmount(0.2, true)).toBe('1');
  });

  it('a_phone_virtual_list_draws_its_pull_over_the_rows_without_moving_them', () => {
    as('android', () => (
      <VirtualList aria-label="Tasks" items={['a', 'b']} key={(s) => s} viewport={400} pull={{ progress: 1, refreshing: false, label: 'Reconnecting' }}>
        {(s) => <Text>{s()}</Text>}
      </VirtualList>
    ));
    const kids = [...(q('[data-kit="virtual-list"]')?.children ?? [])].map((c) => c.getAttribute('data-kit'));
    expect(kids).toEqual(['pull-refresh', 'virtual-spacer', 'virtual-row', 'virtual-row', 'virtual-spacer']);
    expect(q('[data-kit="pull-refresh"]')?.dataset['part']).toBe('ui.virtual-list.pull');
    expect(q('[data-kit="pull-refresh"]')?.hasAttribute('data-armed')).toBe(true);
  });
});

describe('the sheets have every step-4 variant', () => {
  const files = import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true });
  const KITS = ['list-row', 'list-row-lead', 'swipe-band', 'section-head', 'settings-group-head', 'settings-group-body', 'card', 'banner', 'pill', 'skeleton', 'empty-state', 'pull-refresh'];
  for (const [path, css] of Object.entries(files)) {
    const look = path.replace(/^.*\/|\.css$/g, '');
    it(`${look}_styles_rows_and_surfaces`, () => {
      for (const kit of KITS) expect(css, `${look} ${kit}`).toContain(`:root[data-platform='${look}'] [data-kit='${kit}']`);
      if (look === 'ios') for (const kit of ['list-row-chevron', 'swipe-action', 'section-footer', 'settings-group-footer', 'pull-tick']) expect(css, kit).toContain(`[data-kit='${kit}']`);
      if (look === 'android') expect(css).toContain("[data-kit='pull-disc']");
    });
  }

  it('a_dragged_row_tracks_the_finger_with_no_transition_in_either_look', () => {
    for (const [path, css] of Object.entries(files)) {
      const look = path.replace(/^.*\/|\.css$/g, '');
      expect(css, look).toContain(`:root[data-platform='${look}'] [data-kit='swipe'][data-phase='drag'] > [data-kit='swipe-row'] { transition: none; }`);
    }
  });

  it('ios_keeps_separators_on_rows_wrapped_alone_in_a_swipe_or_virtual_row', () => {
    const ios = Object.entries(files).find(([p]) => p.endsWith('ios.css'))?.[1] ?? '';
    expect(ios).not.toContain("[data-platform='ios'] [data-kit='list-row']:last-child::before");
    expect(ios).toContain(":not([data-kit='swipe-row']):not([data-kit='virtual-row']) > [data-kit='list-row']:last-child::before");
  });

  it('row_text_and_band_words_never_wrap_in_either_look', () => {
    for (const css of Object.values(files)) expect(css).not.toMatch(/(list-row-title|swipe-label|pill-label)'\][^{]*\{[^}]*white-space:\s*(normal|pre-wrap)/);
  });
});
