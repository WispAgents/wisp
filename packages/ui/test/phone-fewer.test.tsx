/// <reference types="vite/client" />
// docs/ux/2026-11-18-phone.md: the kit's phone pieces for fewer options showing. The page status pills (#3), the
// reply options chip and sheet (#1), the thinking levels named on a phone (#9), the Frame's pushed page (#2), and the
// rules the phone sheets carry: touch targets (#4), opaque menus (#7), the run line's place (#10). The browser's
// render of the same components is unchanged (no chip, no labels).
import { afterEach, expect, test } from 'vitest';
import { mount, until } from './helpers.tsx';
import { createSignal, Show } from 'solid-js';
import { GestureHostProvider, HarnessPicker, PageStatus, type GestureHost, PlatformLookProvider, ReplyBox, ThinkingSlider, TurnView, WithAct, type Look } from '../src/index.ts';
import { bindAccessory, keyboardInset } from '../src/terminal/accessory.ts';

const sheets = Object.entries(import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true })).map(([p, raw]) => ({
  look: p.replace(/^.*\/|\.css$/g, '') as 'android' | 'ios',
  css: raw.replace(/\/\*[^]*?\*\//g, ''),
}));
const sheet = (look: 'android' | 'ios'): string => sheets.find((s) => s.look === look)?.css ?? '';

afterEach(() => {
  document.body.innerHTML = '';
});

const MODES = [
  { value: 'plan', label: 'Plan', hint: 'Reads and plans' },
  { value: 'edits', label: 'Edits' },
  { value: 'auto', label: 'Full auto', checked: true },
];

function reply(look: Look, picked: string[] = []): HTMLElement {
  return mount(() => (
    <PlatformLookProvider look={() => look}>
      <ReplyBox
        placeholder="Follow up…"
        value=""
        onInput={() => undefined}
        onSend={() => undefined}
        attach="Attach files"
        mode={{ label: 'Full auto', title: 'Full auto', act: { menu: { items: MODES, onSelect: (v) => picked.push(v) } } }}
        runner={{ title: 'Codex · gpt-6-sol\nPick another harness', act: { onClick: () => picked.push('runner') } }}
        team="Delegate: agents the lead can hand parts to"
        teamAct={{ onClick: () => picked.push('team') }}
        goal="Add a goal"
        goalAct={{ onClick: () => picked.push('goal') }}
        later="Start later"
        laterAct={{ onClick: () => picked.push('later') }}
        send="Send"
        phoneOptions={{ options: 'Reply options', mode: 'Permissions', agent: 'Agent', goal: 'Goal' }}
      />
    </PlatformLookProvider>
  ));
}

for (const look of ['android', 'ios'] as const) {
  test(`on_${look}_the_reply_bar_is_attach_the_options_chip_and_send_and_the_sheet_runs_each_tool`, async () => {
    const picked: string[] = [];
    const host = reply(look, picked);
    const bar = host.querySelector('[data-kit="c-bar"]') as HTMLElement;
    expect([...bar.children].map((e) => e.getAttribute('data-part')).filter(Boolean)).toEqual(['composer.attach', 'composer.harness', 'composer.send']);
    const chip = bar.querySelector<HTMLButtonElement>('[data-kit="c-opts"]');
    expect(chip?.getAttribute('aria-label')).toBe('Reply options: Full auto · Codex · gpt-6-sol');
    chip?.click();
    await until(() => document.querySelector('[data-kit="opts-list"]') !== null);
    const rows = (part: string): HTMLButtonElement[] => [...document.querySelectorAll<HTMLButtonElement>(`[data-kit="opts-list"] [data-part="${part}"]`)];
    expect(rows('composer.options.mode').map((b) => b.textContent)).toEqual(['PlanReads and plans', 'Edits', 'Full auto']);
    expect(rows('composer.options.mode').map((b) => b.getAttribute('aria-checked'))).toEqual(['false', 'false', 'true']);
    rows('composer.options.mode')[0]?.click();
    expect(picked).toEqual(['plan']);
    for (const part of ['composer.team', 'composer.goal', 'composer.sendLater']) expect(rows(part), part).toHaveLength(1);
    // a press that leaves the sheet for what it opens: it closes, then runs (after a microtask)
    rows('composer.goal')[0]?.click();
    await until(() => picked.includes('goal'));
    expect(picked).toContain('goal');
  });
}

test('in_the_browser_the_reply_bar_keeps_every_tool_and_no_chip', () => {
  const host = reply('web');
  expect(host.querySelector('[data-kit="c-opts"]')).toBeNull();
  for (const part of ['composer.mode', 'composer.team', 'composer.goal', 'composer.sendLater']) expect(host.querySelector(`[data-part="${part}"]`), part).not.toBeNull();
});

test('page_status_draws_only_what_there_is_one_line_its_open_pane_pressed', () => {
  const hits: string[] = [];
  const host = mount(() => (
    <PageStatus
      label="What this task holds"
      part="phone.task.status"
      items={[
        { id: 'changes', label: '3 files +42 −7', icon: 'file', on: true, onSelect: () => hits.push('changes') },
        { id: 'pr', label: 'Pull request', onSelect: () => hits.push('pr') },
      ]}
    />
  ));
  const pills = [...host.querySelectorAll<HTMLButtonElement>('[data-kit="page-pill"]')];
  expect(pills.map((p) => p.getAttribute('data-part'))).toEqual(['phone.task.status.changes', 'phone.task.status.pr']);
  expect(pills.map((p) => p.getAttribute('aria-pressed'))).toEqual(['true', 'false']);
  pills[1]?.click();
  expect(hits).toEqual(['pr']);
  const empty = mount(() => <PageStatus label="x" items={[]} />);
  expect(empty.querySelector('[data-kit="page-status"]'), 'no empty row').toBeNull();
});

test('the_thinking_levels_are_named_and_tappable_on_a_phone_only', () => {
  const picked: string[] = [];
  const slider = (look: Look): HTMLElement =>
    mount(() => (
      <PlatformLookProvider look={() => look}>
        <ThinkingSlider stops={['default', 'low', 'high']} value="low" onChange={(v) => picked.push(v)} heading="Thinking" label={(v) => v.toUpperCase()} aria={{ label: 'Thinking', valueText: (v) => v }} />
      </PlatformLookProvider>
    ));
  const phone = slider('android');
  const labels = [...phone.querySelectorAll<HTMLButtonElement>('[data-kit="think-label"]')];
  expect(labels.map((l) => l.textContent)).toEqual(['DEFAULT', 'LOW', 'HIGH']);
  expect(labels.map((l) => l.hasAttribute('data-on'))).toEqual([false, true, false]);
  labels[2]?.click();
  expect(picked).toEqual(['high']);
  expect(slider('web').querySelector('[data-kit="think-labels"]'), 'the browser draws none').toBeNull();
});

for (const look of ['android', 'ios'] as const) {
  test(`the_${look}_sheet_hides_the_tab_bar_on_a_pushed_page_and_gives_composer_controls_the_touch_target`, () => {
    const css = sheet(look);
    expect(css).toMatch(new RegExp(`\\[data-kit='frame'\\]\\[data-pushed\\] > \\[data-kit='frame-nav'\\] \\{ display: none; \\}`));
    expect(css).toMatch(/\[data-kit='c-bar'\] > button:not\(\[data-kit='send'\]\)[^{]*\{ min-width: var\(--tap-min\); min-height: var\(--tap-min\); \}/);
    expect(css).toMatch(/\[data-kit='row-run'\]\[data-local\] > \[data-kit='ir-w'\] \{ display: none; \}/);
    // #5: New task's setup is a list (a row each) with attach and More on its last line
    expect(css).toMatch(/\[data-kit='frame'\]\[data-behind\] \[data-kit='launcher'\] \[data-kit='l-bar'\] \{\s*display: grid;/);
    expect(css).toMatch(/\[data-kit='l-bar'\] > \[data-a='where'\]::after/);
    expect(css).toMatch(/\[data-kit='turn'\]:not\(\[data-latest\]\) \[data-kit='trail-bar'\] > \[data-kit='tb-tools'\] \{ display: none; \}/);
  });
}

test('the_ios_menu_and_popover_sit_on_an_opaque_base_under_the_material', () => {
  expect(sheet('ios')).toMatch(/\[data-kit='popover'\] \{ background: linear-gradient\(var\(--chrome\), var\(--chrome\)\), var\(--surface-c1\); \}/);
});

// QA r6 phone #7: Delegate from the reply's options is its own page on a phone, not the browser's popover over the
// sheet: the options give way to it, and closing it brings them back (as Agent › does)
for (const look of ['android', 'ios'] as const) {
  test(`on_${look}_delegate_from_the_options_is_a_page_and_closing_it_brings_the_options_back`, async () => {
    mount(() => (
      <PlatformLookProvider look={() => look}>
        <ReplyBox
          placeholder="Follow up…"
          value=""
          onInput={() => undefined}
          onSend={() => undefined}
          attach="Attach files"
          mode={{ label: 'Full auto', title: 'Full auto', act: { menu: { items: MODES, onSelect: () => undefined } } }}
          runner={{ title: 'Codex · gpt-6-sol', act: { onClick: () => undefined } }}
          team="Delegate"
          teamAct={{ panel: { label: 'Delegate', part: 'composer.team.panel', content: () => <p data-part="test.presets">Presets</p> } }}
          goal="Add a goal"
          later="Start later"
          send="Send"
          phoneOptions={{ options: 'Reply options', mode: 'Permissions', agent: 'Agent' }}
        />
      </PlatformLookProvider>
    ));
    const optionsOpen = (): boolean => document.querySelector('[data-kit="opts-list"]') !== null;
    const presets = (): Element | null => document.querySelector('[data-part="test.presets"]');
    document.querySelector<HTMLButtonElement>('[data-kit="c-opts"]')?.click();
    await until(optionsOpen);
    document.querySelector<HTMLButtonElement>('[data-kit="opts-list"] [data-part="composer.team"]')?.click();
    await until(() => presets() !== null && !optionsOpen());
    expect(presets(), 'the Delegate page').not.toBeNull();
    expect(presets()?.closest('[data-kit="popover"]'), 'not the popover').toBeNull();
    expect(presets()?.closest('[role="dialog"]'), 'a sheet of its own').not.toBeNull();
    expect(optionsOpen(), 'the options gave way').toBe(false);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    presets()?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await until(() => presets() === null && optionsOpen());
    expect(optionsOpen(), 'back on the options').toBe(true);
  });

  // the same rule everywhere a panel opens (New task's Delegate row): on a phone it's a sheet, never the popover
  test(`on_${look}_a_controls_panel_opens_as_a_sheet`, async () => {
    const host = mount(() => (
      <PlatformLookProvider look={() => look}>
        <WithAct
          act={{ panel: { label: 'Delegate', part: 'composer.team.panel', content: () => <p data-part="test.panel">Presets</p> } }}
          render={(trigger) => (
            <button type="button" {...trigger()} data-part="test.open">
              Delegate
            </button>
          )}
        />
      </PlatformLookProvider>
    ));
    const panel = (): Element | null => document.querySelector('[data-part="test.panel"]');
    host.querySelector<HTMLButtonElement>('[data-part="test.open"]')?.click();
    await until(() => panel() !== null);
    expect(panel()?.closest('[data-kit="popover"]')).toBeNull();
    expect(panel()?.closest('[role="dialog"]')).not.toBeNull();
  });
}

// QA r6 phone #11 (UX #5): Android's New task confirm (Start) is a filled M3 button; the bar's other confirms (Done in
// selection) stay text buttons
test('on_android_the_compose_bars_start_is_a_filled_button', () => {
  const css = sheet('android');
  const filled = css.match(/\[data-kit='app-bar'\]\[data-kind='compose'\] \[data-kit='app-bar-confirm'\] > \[data-kit='button'\] \{([^}]*)\}/)?.[1] ?? '';
  expect(filled).toMatch(/background: var\(--primary\);/);
  expect(filled).toMatch(/color: var\(--on-primary\);/);
  expect(css).toMatch(/\[data-kit='app-bar'\]\[data-kind='compose'\] \[data-kit='app-bar-confirm'\] > \[data-kit='button'\]:disabled \{[^}]*background: color-mix/);
});

// QA r6 phone #8: every composer target is the platform's touch size as drawn, not only by an invisible hit area: Send's
// own box is --tap-min (its circle drawn inside it), and New task's harness row fills its row's height like the rows
// above and below it
for (const look of ['android', 'ios'] as const) {
  test(`on_${look}_send_and_the_new_task_harness_row_are_touch_sized`, () => {
    const css = sheet(look);
    const send = css.match(/\[data-kit='reply-box'\] \[data-kit='send'\],\s*:root\[data-platform='\w+'\] \[data-kit='launcher'\] \[data-kit='send'\] \{([^}]*)\}/)?.[1] ?? '';
    expect(send).toMatch(/width: var\(--tap-min\);/);
    expect(send).toMatch(/height: var\(--tap-min\);/);
    expect(send).toMatch(/background-clip: content-box;/);
    expect(css).not.toMatch(/\[data-kit='send'\]::after/);
    // the disabled fill keeps the circle (a `background:` shorthand would reset the clip)
    expect(css).not.toMatch(/\[data-kit='send'\]:disabled \{\s*background:/);
    expect(css).toMatch(/\[data-kit='frame'\]\[data-behind\] \[data-kit='l-bar'\] > \[data-kit='runners'\] \[data-kit='runner-btn'\] \{[^}]*min-height: var\(--tap-min\);/);
  });
}

// #13: the Shell's key row rides the keyboard: its height from the visual viewport, kept on the row as --k-kb
test('the_key_rows_keyboard_inset_follows_the_visual_viewport_and_its_binding_cleans_up', () => {
  expect(keyboardInset({ innerHeight: 844, visualViewport: null })).toBe(0);
  expect(keyboardInset({ innerHeight: 844, visualViewport: { height: 844, offsetTop: 0 } as never })).toBe(0);
  expect(keyboardInset({ innerHeight: 844, visualViewport: { height: 508, offsetTop: 0 } as never }), 'the keyboard covers 336').toBe(336);
  expect(keyboardInset({ innerHeight: 844, visualViewport: { height: 508, offsetTop: 40 } as never }), 'scrolled: what is under the visual viewport').toBe(296);
  const fns: Record<string, () => void> = {};
  const vv = { height: 844, offsetTop: 0, addEventListener: (t: string, fn: () => void) => void (fns[t] = fn), removeEventListener: (t: string) => void delete fns[t] };
  const el = document.createElement('div');
  const stop = bindAccessory(el, { innerHeight: 844, visualViewport: vv });
  expect(el.style.getPropertyValue('--k-kb')).toBe('0px');
  vv.height = 500;
  fns['resize']?.();
  expect(el.style.getPropertyValue('--k-kb')).toBe('344px');
  stop();
  expect(Object.keys(fns)).toEqual([]);
  expect(el.style.getPropertyValue('--k-kb')).toBe('');
});

// #8: an older answer's actions are a ⋯ menu on a phone, the same menu a long-press on the answer opens; the latest keeps
// its icons, the browser draws every icon
test('an_answers_actions_are_a_menu_on_a_phone_the_latest_keeping_its_icons_and_the_browser_every_icon', async () => {
  const hits: string[] = [];
  const tools = [
    { id: 'notes', icon: 'text' as const, label: 'Agent notes', act: { onClick: () => hits.push('notes') } },
    { id: 'fork', icon: 'fork' as const, label: 'Fork from this turn', act: { onClick: () => hits.push('fork') } },
  ];
  const turn = (look: Look, latest: boolean): HTMLElement =>
    mount(() => (
      <PlatformLookProvider look={() => look}>
        <TurnView id={latest ? 't2' : 't1'} prompt="Hey" harness="Codex" time="22:31" timeTitle="22:31" acts={[]} actsTitle="" latest={latest} stats={[]} tools={tools} />
      </PlatformLookProvider>
    ));
  const older = turn('android', false);
  expect(older.querySelector('[data-part="turn.more.trigger"]'), 'the ⋯').not.toBeNull();
  expect(older.querySelector('[data-kit="turn"]')?.getAttribute('data-gestures') ?? older.querySelector('[data-kit="turn"]')?.outerHTML, 'the long-press is marked').toContain('turn.long-press');
  (older.querySelector('[data-part="turn.more.trigger"]') as HTMLElement).click();
  await until(() => [...document.querySelectorAll('[data-kit="menu-item"]')].some((e) => e.textContent?.includes('Fork from this turn')));
  [...document.querySelectorAll<HTMLElement>('[data-kit="menu-item"]')].find((e) => e.textContent?.includes('Fork from this turn'))?.click();
  await until(() => hits.includes('fork'));
  expect(hits).toEqual(['fork']);
  // the latest answer has its ⋯ too (its icons also stay out: the stylesheet folds only an older one's)
  expect(turn('android', true).querySelector('[data-part="turn.more.trigger"]'), 'the latest has its ⋯ too').not.toBeNull();
  expect(turn('web', false).querySelector('[data-part="turn.more.trigger"]'), 'the browser draws every icon').toBeNull();
});


// #9: the agent's page is pushed from the reply's options: Agent closes the sheet and opens the picker; the picker's
// Back (a phone draws it) closes it, and the sheet comes back
test('the_agent_page_is_pushed_from_the_options_and_back_pops_to_them', async () => {
  const [pickerOpen, setPickerOpen] = createSignal(false);
  let closed = 0;
  mount(() => (
    <PlatformLookProvider look={() => 'ios'}>
      <ReplyBox
        placeholder="Follow up…"
        value=""
        onInput={() => undefined}
        onSend={() => undefined}
        attach="Attach files"
        mode={{ label: 'Full auto', title: 'Full auto', act: { menu: { items: MODES, onSelect: () => undefined } } }}
        runner={{ title: 'Codex · gpt-6-sol', open: pickerOpen(), act: { onClick: () => setPickerOpen(true) } }}
        team="Delegate"
        goal="Add a goal"
        later="Start later"
        send="Send"
        phoneOptions={{ options: 'Reply options', mode: 'Permissions', agent: 'Agent' }}
      />
      <Show when={pickerOpen()}>
        <HarnessPicker
          anchor={() => document.querySelector('[data-part="composer.harness"]')}
          label="Harness and model"
          head="Harness"
          rows={[{ id: 'codex', name: 'Codex', sub: 'gpt-6-sol' }]}
          inUse="codex"
          inUseLabel="In use"
          focused="codex"
          onPreview={() => undefined}
          onUse={() => undefined}
          pane={{ label: 'Model', groups: [{ items: [{ id: '', label: 'Default' }] }], value: '', onPick: () => undefined, empty: 'No models' }}
          onClose={() => (closed++, setPickerOpen(false))}
        />
      </Show>
    </PlatformLookProvider>
  ));
  const sheetOpen = (): boolean => document.querySelector('[data-kit="opts-list"]') !== null;
  document.querySelector<HTMLButtonElement>('[data-kit="c-opts"]')?.click();
  await until(sheetOpen);
  document.querySelector<HTMLButtonElement>('[data-part="composer.options.agent"]')?.click();
  await until(() => pickerOpen() && !sheetOpen());
  expect(pickerOpen(), 'the agent page').toBe(true);
  const back = document.querySelector<HTMLButtonElement>('[data-part="runner.picker.back"]');
  expect(back?.textContent).toBe('Back');
  back?.click();
  await until(() => !pickerOpen() && sheetOpen());
  expect(closed).toBe(1);
  expect(sheetOpen(), 'back on the options').toBe(true);
});

// QA r6 #15: a real touch long-press (pointerdown held past the timer, no move) on an answer opens its menu
test('a_touch_long_press_on_an_answer_opens_its_menu', async () => {
  const hits: string[] = [];
  const ticks: string[] = [];
  const host = { haptic: (k: string) => void ticks.push(k), actions: () => () => undefined } as unknown as GestureHost;
  const box = mount(() => (
    <GestureHostProvider host={host}>
      <PlatformLookProvider look={() => 'android'}>
        <TurnView id="t1" prompt="Hey" harness="Codex" time="22:31" timeTitle="22:31" acts={[]} actsTitle="" latest stats={[]} tools={[{ id: 'fork', icon: 'fork', label: 'Fork from the held answer', act: { onClick: () => hits.push('fork') } }]} />
      </PlatformLookProvider>
    </GestureHostProvider>
  ));
  const turn = box.querySelector('[data-kit="turn"]') as HTMLElement;
  expect(turn.getAttribute('data-gesture')).toContain('turn.long-press');
  const ask = turn.querySelector('[data-kit="ask"]') as HTMLElement;
  ask.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, pointerType: 'touch', isPrimary: true, clientX: 120, clientY: 80 }));
  // held past the long-press time (the haptic ticks), then the finger lifts: the menu opens and stays
  await until(() => ticks.length > 0, 2000);
  expect(ticks).toEqual(['tap']);
  ask.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, pointerType: 'touch', isPrimary: true }));
  await until(() => [...document.querySelectorAll('[data-kit="menu-item"]')].some((e) => e.textContent?.includes('Fork from the held answer') && e.closest('[data-kit="menu"]')?.getAttribute('data-state') === 'open'), 2000);
  // the newest menu (an earlier test's may still be in the page)
  const fork = [...document.querySelectorAll<HTMLElement>('[data-kit="menu-item"]')].filter((e) => e.textContent?.includes('Fork from the held answer')).at(-1);
  expect(fork, 'the menu opened from the hold').toBeDefined();
  expect(fork?.closest('[data-kit="menu"]')?.getAttribute('data-state'), 'still open after the finger lifted').toBe('open');
  fork?.click();
  await until(() => hits.includes('fork'));
  expect(hits).toEqual(['fork']);
});

