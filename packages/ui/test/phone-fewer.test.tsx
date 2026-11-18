/// <reference types="vite/client" />
// docs/ux/2026-11-18-phone.md: the kit's phone pieces for fewer options showing. The page status pills (#3), the
// reply options chip and sheet (#1), the thinking levels named on a phone (#9), the Frame's pushed page (#2), and the
// rules the phone sheets carry: touch targets (#4), opaque menus (#7), the run line's place (#10). The browser's
// render of the same components is unchanged (no chip, no labels).
import { afterEach, expect, test } from 'vitest';
import { mount, until } from './helpers.tsx';
import { PageStatus, PlatformLookProvider, ReplyBox, ThinkingSlider, type Look } from '../src/index.ts';

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
    expect(css).toMatch(/\[data-kit='turn'\]:not\(\[data-latest\]\) \[data-kit='trail-bar'\] > \[data-kit='tb-tools'\]:not\(\[data-open\]\) \{ display: none; \}/);
  });
}

test('the_ios_menu_and_popover_sit_on_an_opaque_base_under_the_material', () => {
  expect(sheet('ios')).toMatch(/\[data-kit='popover'\] \{ background: linear-gradient\(var\(--chrome\), var\(--chrome\)\), var\(--surface-c1\); \}/);
});
