/// <reference types="vite/client" />
// ADR-0065 build step 6: the controls' phone variants (design/mobile.md §A.5–§A.7). Under `data-platform` (here a
// PlatformLookProvider) Button, IconButton, Switch, Tabs, Input, TextArea, Field and Select draw Material 3 on Android
// and HIG on iOS: shapes, sizes and states from the look's stylesheet and tokens, the few structural differences (the
// M3 switch's handle icon, the floated label, a long Select's sheet) from the components. The browser's render is byte
// for byte what it was before the variants landed (test/goldens/platform-browser-controls.json), and the browser never
// fetches the phone halves.
import { describe, expect, it, vi } from 'vitest';
import goldenRaw from './goldens/platform-browser-controls.json?raw';
import { createSignal, type JSX } from 'solid-js';
import { frame, mount, tick } from './helpers.tsx';
import { controlScenes, FEW, MANY } from './platform-scenes-controls.tsx';
import { Field, GestureHostProvider, Input, PlatformLookProvider, SHEET_AFTER, Select, Switch, Tabs, TextArea, selectSheet, type GestureHost, type Haptic, type Look } from '../src/index.ts';
import { phoneOverlays } from '../src/controls/phone-part.ts';

const GOLDEN = JSON.parse(goldenRaw) as Record<string, string>;
const sheets = Object.entries(import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true })).map(([p, raw]) => ({
  look: p.replace(/^.*\/|\.css$/g, '') as 'android' | 'ios',
  css: raw.replace(/\/\*[^]*?\*\//g, ''),
}));
const sheet = (look: 'android' | 'ios'): string => sheets.find((s) => s.look === look)?.css ?? '';
const tokenFiles = import.meta.glob<string>('../../tokens/tokens/platform/*.json', { query: '?raw', import: 'default', eager: true });
const token = (look: 'android' | 'ios', name: string): string | undefined => {
  const raw = Object.entries(tokenFiles).find(([p]) => p.endsWith(`${look}.json`))?.[1] ?? '{}';
  const set = JSON.parse(raw) as { static: Record<string, { value: string }> };
  return set.static[name]?.value;
};
/** the bodies of every rule whose selector list holds exactly `sel` */
const body = (css: string, sel: string): string =>
  [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .filter((m) => (m[1] ?? '').split(',').some((s) => s.trim() === sel))
    .map((m) => m[2] ?? '')
    .join('\n');
/** the sheet's last reduced-motion block: step 6's */
// every prefers-reduced-motion block of a sheet, joined: a sheet may have several (the controls', the pages' transitions)
const reduced = (css: string): string => css.split('@media (prefers-reduced-motion: reduce)').slice(1).join('\n');

function host(): GestureHost & { readonly ticks: Haptic[] } {
  const ticks: Haptic[] = [];
  return { ticks, haptic: (k) => void ticks.push(k) };
}
const as = (look: Look, view: () => JSX.Element, h: GestureHost = host()): HTMLElement =>
  mount(() => (
    <PlatformLookProvider look={() => look}>
      <GestureHostProvider host={h}>{view()}</GestureHostProvider>
    </PlatformLookProvider>
  ));
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const qa = (sel: string): HTMLElement[] => [...document.querySelectorAll<HTMLElement>(sel)];
const noop = (): void => {};

describe('the browser look is unchanged', () => {
  it('a_browser_render_of_every_control_is_byte_identical_to_before', () => {
    for (const [name, view] of controlScenes()) {
      const h = mount(view);
      expect(h.innerHTML, name).toBe(GOLDEN[name]);
      h.remove();
    }
  });

  it('a_browser_long_select_opens_its_menu_and_never_fetches_the_phone_half', async () => {
    const h = host();
    as('web', () => <Select items={MANY} value="studio" onChange={noop} aria-label="Computer" />, h);
    q('[data-kit="select-trigger"]')?.click();
    await tick(20);
    expect(phoneOverlays.ready()).toBeUndefined();
    expect(q('[data-kit="select-sheet"]')).toBeNull();
    expect(q('[data-kit="select-trigger"]')?.getAttribute('aria-expanded')).toBe('true');
    expect(h.ticks).toEqual([]);
  });

  it('a_browser_switch_and_tabs_tick_no_haptic_and_the_switch_has_no_handle_icon', () => {
    const h = host();
    const [on, setOn] = createSignal(true);
    as('web', () => (
      <>
        <Switch checked={on()} onChange={setOn} aria-label="On" />
        <Tabs items={[{ value: 'a', label: 'A' }, { value: 'b', label: 'B' }]} />
      </>
    ), h);
    expect(q('[data-kit="switch-thumb"]')?.innerHTML).toBe('');
    q('[data-kit="switch"]')?.click();
    qa('[data-kit="tab"]')[1]?.click();
    expect(on()).toBe(false);
    expect(h.ticks).toEqual([]);
  });

  it('a_browser_field_never_floats_its_label', () => {
    as('web', () => <Field label="Name"><Input value="x" /></Field>);
    expect(q('[data-kit="field"]')?.hasAttribute('data-float')).toBe(false);
  });

  it('the_browser_stylesheet_carries_none_of_the_control_variants', () => {
    const files = import.meta.glob<string>('../src/css/*.css', { query: '?raw', import: 'default', eager: true });
    for (const [path, raw] of Object.entries(files)) {
      const css = raw.replace(/\/\*[^]*?\*\//g, '');
      expect(css, path).not.toMatch(/data-platform|'select-sheet|data-float|--switch-thumb|--seg-|--tab-indicator|--field-h|--button-h|--icon-button/);
    }
  });
});

describe('Button and IconButton', () => {
  it('android_maps_every_variant_to_an_m3_button_and_ios_to_a_hig_one', () => {
    // M3: filled, tonal (the base), text, an outlined filter chip; HIG: filled, tinted, gray (the base), plain
    const a = sheet('android');
    expect(body(a, ":root[data-platform='android'] :where([data-variant='primary'])[data-kit='button']")).toContain('background: var(--primary)');
    expect(body(a, ":root[data-platform='android'] [data-kit='button']")).toContain('background: var(--primary-container)');
    expect(body(a, ":root[data-platform='android'] :where([data-variant='ghost'])[data-kit='button']")).toContain('background: transparent');
    expect(body(a, ":root[data-platform='android'] :where([data-variant='chip'])[data-kit='button']")).toContain('border: var(--hairline) solid var(--outline)');
    expect(body(a, ":root[data-platform='android'] :where([data-variant='danger'])[data-kit='button']")).toContain('background: var(--error)');
    const i = sheet('ios');
    expect(body(i, ":root[data-platform='ios'] :where([data-variant='primary'])[data-kit='button']")).toContain('background: var(--primary)');
    expect(body(i, ":root[data-platform='ios'] :where([data-variant='accent'])[data-kit='button']")).toContain('color-mix(in srgb, var(--primary) 15%, transparent)');
    expect(body(i, ":root[data-platform='ios'] [data-kit='button']")).toContain('background: var(--fill)');
    expect(body(i, ":root[data-platform='ios'] :where([data-variant='ghost'])[data-kit='button']")).toContain('background: transparent');
    expect(body(a, ":root[data-platform='android'] :where([data-variant='primary'])[data-kit='icon-button']")).toContain('background: var(--primary)');
    expect(body(a, ":root[data-platform='android'] [data-kit='icon-button']")).toContain('background: transparent');
    expect(body(i, ":root[data-platform='ios'] [data-kit='icon-button']")).toContain('color: var(--primary)');
  });

  it('button_and_icon_button_sizes_come_from_the_platform_tokens', () => {
    const a = sheet('android');
    expect(body(a, ":root[data-platform='android'] [data-kit='button']")).toContain('height: var(--button-h)');
    expect(body(a, ":root[data-platform='android'] :where([data-size='sm'])[data-kit='button']")).toContain('height: var(--button-h-sm)');
    expect(body(a, ":root[data-platform='android'] :where([data-size='lg'])[data-kit='button']")).toContain('height: var(--button-h-lg)');
    expect(body(a, ":root[data-platform='android'] [data-kit='icon-button']")).toContain('width: var(--icon-button)');
    expect([token('android', 'button-h-sm'), token('android', 'button-h'), token('android', 'button-h-lg'), token('android', 'icon-button')]).toEqual(['32px', '40px', '56px', '40px']);
    const i = sheet('ios');
    expect(body(i, ":root[data-platform='ios'] [data-kit='button']")).toContain('height: var(--button-h-sm)');
    expect(body(i, ":root[data-platform='ios'] :where([data-size='xs'])[data-kit='button']")).toContain('height: var(--button-h-xs)');
    expect(body(i, ":root[data-platform='ios'] :where([data-size='lg'])[data-kit='button']")).toContain('height: var(--button-h)');
    expect([token('ios', 'button-h-xs'), token('ios', 'button-h-sm'), token('ios', 'button-h')]).toEqual(['28px', '34px', '50px']);
  });

  it('a_smaller_visible_control_grows_its_touch_target_to_48dp_or_44pt', () => {
    expect(token('android', 'tap-min')).toBe('48px');
    expect(token('ios', 'tap-min')).toBe('44px');
    for (const s of sheets)
      for (const kit of ["[data-kit='button']:not([data-variant='link'])::before", "[data-kit='icon-button']::before"])
        expect(body(s.css, `:root[data-platform='${s.look}'] ${kit}`), `${s.look} ${kit}`).toContain('inset: min(var(--space-0), calc((100% - var(--tap-min)) / 2))');
    for (const s of sheets) expect(body(s.css, `:root[data-platform='${s.look}'] [data-kit='switch']::after`), s.look).toContain('var(--tap-min)');
    expect(body(sheet('ios'), ":root[data-platform='ios'] [data-kit='tabs'] [data-kit='tab']::after")).toContain('var(--tap-min)');
  });

  it('android_ripples_from_the_centre_while_held_and_reduced_motion_keeps_only_the_state_layer', () => {
    const a = sheet('android');
    const held = body(a, ":root[data-platform='android'] [data-kit='button']:not([data-variant='link']):active::after");
    expect(held).toContain('background-size: 250% 250%');
    expect(held).toContain('background-color: color-mix(in srgb, currentColor calc(var(--state-pressed) * 100%), transparent)');
    expect(body(a, ":root[data-platform='android'] [data-kit='icon-button']::after")).toContain('radial-gradient(circle closest-side');
    const calm = reduced(a);
    expect(calm).toContain(":root[data-platform='android'] [data-kit='button']:not([data-variant='link']):active::after");
    expect(calm).toMatch(/background-size: 0% 0%; transition: none;/);
    // iOS has no ripple: a press dims the control (the highlight)
    expect(sheet('ios')).not.toContain('radial-gradient');
    expect(body(sheet('ios'), ":root[data-platform='ios'] [data-kit='button']:active")).toContain('opacity: calc(1 - var(--state-pressed) * 2)');
  });

  it('every_control_has_hover_focus_pressed_and_disabled_states_in_both_looks', () => {
    for (const s of sheets) {
      const r = `:root[data-platform='${s.look}'] `;
      expect(s.css, `${s.look} hover`).toMatch(/@media \(hover: hover\) \{[^@]*\[data-kit='button'\]:hover/);
      for (const kit of ['button', 'icon-button']) {
        expect(body(s.css, `${r}[data-kit='${kit}']:focus-visible`), `${s.look} ${kit} focus`).toContain('outline: var(--size-ring) solid var(--primary)');
        expect(body(s.css, `${r}[data-kit='${kit}']:disabled`), `${s.look} ${kit} disabled`).toContain('opacity: var(--state-disabled)');
        expect(body(s.css, `${r}[data-kit='${kit}']:active`), `${s.look} ${kit} pressed: no scale`).toContain('transform: none');
      }
      expect(body(s.css, `${r}[data-kit='switch'][data-disabled]`), `${s.look} switch disabled`).toContain('var(--state-disabled)');
      expect(body(s.css, `${r}[data-kit='switch']:focus-visible`), `${s.look} switch focus`).toContain('var(--primary)');
      expect(body(s.css, `${r}[data-kit='tabs'] [data-kit='tab'][data-disabled]`), `${s.look} tab disabled`).toContain('var(--state-disabled)');
      expect(body(s.css, `${r}[data-kit='input']:disabled`), `${s.look} input disabled`).toContain('var(--state-disabled)');
      expect(body(s.css, `${r}[data-kit='input']:focus`), `${s.look} input focus`).toContain('var(--primary)');
    }
    expect(token('ios', 'state-disabled')).toBe('0.3');
    expect(token('android', 'state-disabled')).toBe('0.38');
  });

  it('a_generic_control_rule_never_outranks_the_bar_dialog_and_banner_rules_of_steps_3_to_5', () => {
    // one look + one kit name: (0,3,0); variants and sizes ride in :where(), so `[data-kit='dialog-foot'] > [data-kit='button']` still wins
    for (const s of sheets) {
      const start = s.css.indexOf(`:root[data-platform='${s.look}'] [data-kit='button'] {`);
      expect(start, s.look).toBeGreaterThan(0);
      for (const m of s.css.slice(start).matchAll(/:root\[data-platform='\w+'\] \[data-kit='(button|icon-button)'\]((?:\[[^\]]+\])+)/g)) expect(m[2], `${s.look}: ${m[0]}`).toMatch(/^\[data-(state|loading|disabled)/);
    }
  });
});

describe('Switch', () => {
  it('android_draws_the_m3_handle_with_its_check_only_when_on', () => {
    const [on, setOn] = createSignal(false);
    as('android', () => <Switch checked={on()} onChange={setOn} aria-label="Stay connected" />);
    expect(q('[data-kit="switch-thumb"] [data-icon="check"]')).toBeNull();
    q('[data-kit="switch"]')?.click();
    expect(on()).toBe(true);
    expect(q('[data-kit="switch-thumb"] [data-icon="check"]')).not.toBeNull();
    expect(q('[data-kit="switch"]')?.getAttribute('aria-checked')).toBe('true');
  });

  it('ios_draws_a_bare_thumb_and_both_phones_tick_the_picker_haptic_as_it_flips', () => {
    for (const look of ['android', 'ios'] as const) {
      const h = host();
      const [on, setOn] = createSignal(true);
      const el = as(look, () => <Switch checked={on()} onChange={setOn} aria-label="On" />, h);
      if (look === 'ios') expect(el.querySelector('[data-kit="switch-thumb"]')?.innerHTML).toBe('');
      el.querySelector<HTMLElement>('[data-kit="switch"]')?.click();
      expect(h.ticks, look).toEqual(['select']);
      expect(on()).toBe(false);
      el.remove();
    }
  });

  it('the_switch_is_52_by_32_on_android_and_51_by_31_on_ios_from_tokens', () => {
    expect([token('android', 'switch-w'), token('android', 'switch-h'), token('android', 'switch-thumb'), token('android', 'switch-thumb-on'), token('android', 'switch-thumb-press')]).toEqual(['52px', '32px', '16px', '24px', '28px']);
    expect([token('ios', 'switch-w'), token('ios', 'switch-h'), token('ios', 'switch-thumb')]).toEqual(['51px', '31px', '27px']);
    for (const s of sheets) {
      const b = body(s.css, `:root[data-platform='${s.look}'] [data-kit='switch']`);
      expect(b, s.look).toContain('width: var(--switch-w)');
      expect(b, s.look).toContain('height: var(--switch-h)');
    }
  });
});

describe('Tabs', () => {
  it('a_tab_change_on_a_phone_ticks_the_picker_haptic_and_keeps_the_tabs_roles', async () => {
    for (const look of ['android', 'ios'] as const) {
      const h = host();
      const picked: string[] = [];
      const el = as(look, () => <Tabs items={[{ value: 'all', label: 'All' }, { value: 'docs', label: 'Docs' }]} variant="seg" onChange={(v) => picked.push(v)} />, h);
      const tabs = [...el.querySelectorAll<HTMLElement>('[data-kit="tab"]')];
      expect(tabs.map((t) => t.getAttribute('role'))).toEqual(['tab', 'tab']);
      tabs[0]?.focus();
      await tick(10);
      document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true, cancelable: true }));
      await tick(10);
      expect(picked, look).toEqual(['docs']);
      expect(h.ticks, look).toEqual(['select']);
      expect(el.querySelector('[data-kit="tabs"]')?.getAttribute('data-variant')).toBe('seg');
      el.remove();
    }
  });

  it('android_draws_line_as_primary_tabs_and_pill_or_seg_as_secondary_ones', () => {
    const a = sheet('android');
    const primary = body(a, ":root[data-platform='android'] [data-kit='tabs'][data-variant='line'] [data-kit='tab'][data-selected]::after");
    expect(primary).toContain('height: var(--tab-indicator)');
    expect(primary).toContain('left: var(--tab-pad)');
    const secondary = body(a, ":root[data-platform='android'] [data-kit='tabs'][data-variant='seg'] [data-kit='tab'][data-selected]::after");
    expect(secondary).toContain('height: var(--tab-indicator-sec)');
    expect(secondary).toContain('left: var(--space-0)');
    expect(body(a, ":root[data-platform='android'] [data-kit='tabs'] > [data-kit='tab-list']")).toContain('height: var(--tab-h)');
    expect([token('android', 'tab-h'), token('android', 'tab-indicator'), token('android', 'tab-indicator-sec')]).toEqual(['48px', '3px', '2px']);
  });

  it('ios_draws_every_variant_as_a_segmented_control', () => {
    const i = sheet('ios');
    const track = body(i, ":root[data-platform='ios'] [data-kit='tabs'] > [data-kit='tab-list']");
    expect(track).toContain('height: var(--seg-h)');
    expect(track).toContain('background: var(--fill)');
    expect(body(i, ":root[data-platform='ios'] [data-kit='tabs'] [data-kit='tab'][data-selected]")).toContain('background: var(--seg-thumb)');
    // the browser's line underline is undone, not drawn over the segment
    expect(body(i, ":root[data-platform='ios'] [data-kit='tabs'] [data-kit='tab'][data-selected]::after")).toContain('background: none');
    expect(token('ios', 'seg-h')).toBe('32px');
  });
});

describe('Input, TextArea and Field', () => {
  it('android_floats_a_fields_label_into_the_outline_round_a_text_control_only', () => {
    as('android', () => (
      <>
        <Field label="Name" id="f-input"><Input value="x" /></Field>
        <Field label="Notes" id="f-area"><TextArea value="y" /></Field>
        <Field label="Harness" id="f-select"><Select items={FEW} value="codex" onChange={noop} /></Field>
        <Field label="Stay connected" id="f-switch"><Switch checked onChange={noop} /></Field>
      </>
    ));
    expect(qa('[data-kit="field"]').map((f) => f.hasAttribute('data-float'))).toEqual([true, true, true, false]);
    expect(q('[for="f-input"]')?.textContent).toBe('Name');
  });

  it('ios_keeps_the_label_over_a_rounded_field', () => {
    as('ios', () => <Field label="Name"><Input value="x" /></Field>);
    expect(q('[data-kit="field"]')?.hasAttribute('data-float')).toBe(false);
    const i = sheet('ios');
    const field = body(i, ":root[data-platform='ios'] [data-kit='input']");
    expect(field).toContain('height: var(--field-h)');
    expect(field).toContain('border-radius: var(--radius-md)');
    expect(field).toContain('background-color: var(--fill)');
  });

  it('android_fields_are_m3_outlined_56dp_with_a_2dp_focus_outline_that_moves_nothing', () => {
    const a = sheet('android');
    const field = body(a, ":root[data-platform='android'] [data-kit='input']");
    expect(field).toContain('height: var(--field-h)');
    expect(field).toContain('border: var(--hairline) solid var(--outline)');
    expect(field).toContain('border-radius: var(--radius-xs)');
    expect(body(a, ":root[data-platform='android'] [data-kit='input']:focus")).toContain('box-shadow: inset 0 0 0 calc(var(--field-outline-focus) - var(--hairline)) var(--primary)');
    expect(body(a, ":root[data-platform='android'] [data-kit='field'][data-float] > [data-kit='field-label']")).toContain('translate: 0 -50%');
    expect([token('android', 'field-h'), token('android', 'field-outline-focus')]).toEqual(['56px', '2px']);
  });
});

describe('Select', () => {
  it('a_list_longer_than_six_opens_as_a_sheet_on_a_phone_and_never_in_the_browser', () => {
    expect(SHEET_AFTER).toBe(6);
    expect(selectSheet('android', 6)).toBe(false);
    expect(selectSheet('android', 7)).toBe(true);
    expect(selectSheet('ios', 7)).toBe(true);
    expect(selectSheet('web', 50)).toBe(false);
  });

  it('a_short_list_on_a_phone_is_the_same_menu_at_the_button', async () => {
    as('android', () => <Select items={FEW} value="codex" onChange={noop} aria-label="Harness" />);
    const trigger = q('[data-kit="select-trigger"]');
    trigger?.click();
    // the press shows within its frame, whether the machine is already bound or still loading (then the plain button
    // shows it pressed and the machine replays the press once it binds)
    await frame();
    expect(trigger?.getAttribute('aria-expanded')).toBe('true');
    // wait for the machine itself, not for a guessed number of milliseconds
    await import('../src/controls/select-machine.tsx');
    await vi.waitFor(() => expect(q('[data-kit="listbox"]')?.getAttribute('data-state')).toBe('open'));
    expect(q('[data-kit="select-sheet"]')).toBeNull();
  });

  for (const look of ['android', 'ios'] as const) {
    it(`${look}_a_long_list_answers_in_the_frame_then_opens_its_sheet_and_a_choice_closes_it`, async () => {
      const h = host();
      const [value, setValue] = createSignal('studio');
      as(look, () => <Field label="Computer"><Select items={MANY} value={value()} onChange={setValue} /></Field>, h);
      const trigger = q('[data-kit="select-trigger"]');
      trigger?.click();
      // the press shows at once, before the phone half has landed
      expect(trigger?.getAttribute('aria-expanded')).toBe('true');
      expect(trigger?.getAttribute('data-state')).toBe('open');
      await phoneOverlays.load();
      await tick(20);
      const picker = q('[data-kit="select-sheet"]');
      expect(picker?.getAttribute('role')).toBe('radiogroup');
      expect(picker?.getAttribute('aria-label')).toBe('Computer');
      const rows = qa('[data-kit="select-sheet-item"]');
      expect(rows).toHaveLength(MANY.length);
      expect(rows.map((r) => r.getAttribute('aria-checked'))).toEqual(MANY.map((m) => String(m.value === 'studio')));
      if (look === 'android') {
        expect(qa('[data-kit="select-sheet-radio"]')).toHaveLength(MANY.length);
        expect(rows[0]?.querySelector('[data-icon="check"]')).toBeNull();
      } else {
        expect(q('[data-kit="select-sheet-radio"]')).toBeNull();
        expect(rows[0]?.querySelector('[data-icon="check"]')).not.toBeNull();
      }
      expect(q('[data-kit="listbox"]')).toBeNull();
      rows[2]?.click();
      expect(value()).toBe('cloud');
      expect(h.ticks).toEqual(['select']);
      await tick(20);
      expect(q('[data-kit="select-sheet"]')).toBeNull();
      expect(trigger?.getAttribute('aria-expanded')).toBe('false');
    });
  }

  it('a_list_that_grows_past_six_drops_its_machine_and_opens_the_sheet_and_one_that_shrinks_closes_it', async () => {
    const [items, setItems] = createSignal<readonly (typeof MANY)[number][]>(FEW);
    as('android', () => <Select items={items()} value="codex" onChange={noop} aria-label="Model" />);
    const trigger = (): HTMLElement | null => q('[data-kit="select-trigger"]');
    trigger()?.click();
    await import('../src/controls/select-machine.tsx');
    await vi.waitFor(() => expect(q('[data-kit="listbox"]')?.getAttribute('data-state')).toBe('open'));
    trigger()?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    setItems(MANY);
    await tick(10);
    trigger()?.click();
    expect(trigger()?.getAttribute('aria-expanded')).toBe('true');
    await phoneOverlays.load();
    await vi.waitFor(() => expect(q('[data-kit="select-sheet"]')).not.toBeNull());
    setItems(FEW);
    await tick(10);
    expect(q('[data-kit="select-sheet"]')).toBeNull();
    expect(trigger()?.getAttribute('aria-expanded')).toBe('false');
  });

  it('a_dialogs_and_an_alerts_own_press_state_is_not_doubled_by_the_generic_one', () => {
    expect(body(sheet('android'), ":root[data-platform='android'] [data-kit='dialog'] > [data-kit='dialog-foot'] > [data-kit='button']::after")).toContain('background: none');
    expect(body(sheet('ios'), ":root[data-platform='ios'] [data-kit='dialog'][data-alert] > [data-kit='dialog-foot'] > [data-kit='button']:active")).toContain('opacity: 1');
  });

  it('both_looks_style_the_exposed_dropdown_and_the_pull_down_menu', () => {
    expect(body(sheet('android'), ":root[data-platform='android'] [data-kit='listbox']")).toContain('background: var(--surface-c2)');
    expect(body(sheet('android'), ":root[data-platform='android'] [data-kit='select-trigger'][aria-expanded='true'] > [data-kit='icon']")).toContain('rotate(-90deg)');
    expect(body(sheet('ios'), ":root[data-platform='ios'] [data-kit='listbox']")).toContain('background: var(--chrome)');
    expect(body(sheet('android'), ":root[data-platform='android'] [data-kit='select-sheet-item']")).toContain('min-height: var(--row-h-1)');
    expect(body(sheet('ios'), ":root[data-platform='ios'] [data-kit='select-sheet-item']")).toContain('min-height: var(--row-h-1)');
  });
});
