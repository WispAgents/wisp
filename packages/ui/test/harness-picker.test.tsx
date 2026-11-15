// The harness picker (app/HarnessPicker.tsx, v1 runner-picker.js): both panes drawn from props, keys between them.
import { expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { mount } from './helpers.tsx';
import { HarnessPicker, pickerPlace, type HarnessPickerPane } from '../src/app/HarnessPicker.tsx';

const ROWS = [
  { id: 'auto', name: 'Auto', auto: true, sub: 'Picks per task' },
  { id: 'claude', name: 'Claude Code', logo: '/claude.svg', sub: '4 models' },
  { id: 'codex', name: 'Codex', logo: '/codex.svg', sub: 'Checking…' },
  { id: 'cursor', name: 'Cursor', sub: 'Not installed', off: true },
];

const key = (k: string): void => void document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));

function setup() {
  const anchor = document.createElement('button');
  document.body.append(anchor);
  const [focused, setFocused] = createSignal('auto');
  const [goal, setGoal] = createSignal('balanced');
  const [gen, setGen] = createSignal(0);
  const used: string[] = [];
  const picked: string[] = [];
  let closed = 0;
  const pane = (): HarnessPickerPane =>
    gen() >= 0 && focused() === 'auto'
      ? {
          label: 'Goal',
          groups: [
            {
              head: 'Goal',
              items: [
                { id: 'balanced', label: 'Balanced', hint: 'Frontier models · default or medium thinking' },
                { id: 'cheap', label: 'Cheap', hint: 'Lowest cost that still gets it done · low thinking' },
                { id: 'best', label: 'Best', hint: 'Frontier models · high thinking; cost only breaks ties' },
              ],
            },
          ],
          value: goal(),
          onPick: (g) => (setGoal(g), picked.push(g)),
          empty: 'No models match',
          note: { text: 'wisp picks the harness and model for each task.', link: 'How Auto picks', onLink: () => picked.push('how') },
        }
      : {
          label: 'Model',
          groups: [{ items: [{ id: '', label: 'Default', hint: 'Whatever Claude Code is set to' }, { id: 'opus', label: 'Opus 5.5' }] }],
          value: 'opus',
          onPick: (m) => picked.push(m),
          empty: 'No models match',
          thinking: { heading: 'Thinking', stops: ['', 'low', 'high'], value: 'low', onChange: () => undefined, label: (e) => e || 'Default', aria: { label: 'Thinking level', valueText: (e) => e } },
          accounts: { label: 'Account', options: [{ id: '', label: 'Auto' }, { id: 'w', label: 'Work' }, { id: 'o', label: 'Old', off: true }], value: '', onPick: (a) => picked.push(`acct:${a}`) },
        };
  mount(() => (
    <HarnessPicker
      anchor={() => anchor}
      label="Harness and model"
      head="Harness"
      rows={ROWS.map((r) => ({ ...r, sub: gen() > 0 && r.id === 'codex' ? '2 models' : r.sub }))}
      inUse="auto"
      inUseLabel="In use"
      focused={focused()}
      onPreview={setFocused}
      onUse={(id) => (setFocused(id), used.push(id))}
      pane={pane()}
      onClose={() => closed++}
    />
  ));
  return { used, picked, closed: () => closed, focused, refresh: () => setGen((n) => n + 1) };
}

test('both_panes_harnesses_with_logo_and_sub_line_auto_s_goals_with_a_check_and_its_help_line_v1_reference', async () => {
  setup();
  await Promise.resolve();
  const box = document.querySelector('[data-kit="picker"]') as HTMLElement;
  expect(box.getAttribute('role')).toBe('dialog');
  const side = [...box.querySelectorAll('[data-kit="pk-h"]')];
  expect(side.map((b) => b.querySelector('b')?.textContent)).toEqual(['Auto', 'Claude Code', 'Codex', 'Cursor']);
  expect(side.map((b) => b.querySelector('small')?.textContent)).toEqual(['Picks per task', '4 models', 'Checking…', 'Not installed']);
  expect(side[1]?.querySelector('img')?.getAttribute('src')).toBe('/claude.svg');
  expect(side[0]?.querySelector('[data-kit="icon"]')).not.toBeNull();
  // the one in use has the dot, the focused one is lit, a harness that can't run is dimmed
  expect(side.map((b) => b.querySelector('[data-kit="pk-dot"]') !== null)).toEqual([true, false, false, false]);
  expect(side[0]?.getAttribute('aria-selected')).toBe('true');
  expect(side[3]?.hasAttribute('data-off')).toBe(true);
  expect(box.querySelector('[data-kit="pk-side"] > [data-kit="pk-head"]')?.textContent).toBe('Harness');
  // right: GOAL, three goals, Balanced checked, then the help line and its link
  expect(box.querySelector('[data-kit="pk-list"] [data-kit="pk-head"]')?.textContent).toBe('Goal');
  const goals = [...box.querySelectorAll('[data-kit="pk-m"]')];
  expect(goals.map((b) => [b.querySelector('b')?.textContent, b.getAttribute('aria-selected')])).toEqual([
    ['Balanced', 'true'],
    ['Cheap', 'false'],
    ['Best', 'false'],
  ]);
  expect(box.querySelector('[data-kit="pk-note"] a')?.textContent).toBe('How Auto picks');
  // no search, thinking or account for Auto; the picked goal has the focus (v1 focusModels)
  expect(box.querySelector('[data-kit="pk-search"]')).toBeNull();
  expect(box.querySelector('[data-kit="think"]')).toBeNull();
  expect(document.activeElement).toBe(goals[0]);
});

test('a_harness_shows_its_models_thinking_and_accounts_on_the_right', () => {
  const s = setup();
  (document.querySelectorAll('[data-kit="pk-h"]')[1] as HTMLElement).click();
  expect(s.used).toEqual(['claude']);
  const box = document.querySelector('[data-kit="picker"]') as HTMLElement;
  expect([...box.querySelectorAll('[data-kit="pk-m"] b')].map((b) => b.textContent)).toEqual(['Default', 'Opus 5.5']);
  expect(box.querySelector('[data-kit="pk-m"][aria-selected="true"] b')?.textContent).toBe('Opus 5.5');
  expect(box.querySelector('[data-kit="pk-effort"] [data-kit="think"]')).not.toBeNull();
  const accs = [...box.querySelectorAll<HTMLButtonElement>('[data-kit="pk-seg"] > button')];
  expect(accs.map((b) => [b.textContent, b.getAttribute('aria-checked')])).toEqual([
    ['Auto', 'true'],
    ['Work', 'false'],
    ['Old', 'false'],
  ]);
  accs[2]?.click(); // signed out: can't be picked
  accs[1]?.click();
  expect(s.picked).toEqual(['acct:w']);
});

test('keys_arrows_preview_in_the_left_pane_right_and_enter_go_across_left_comes_back_escape_closes', async () => {
  const s = setup();
  await Promise.resolve();
  const side = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('[data-kit="pk-h"]')];
  // from the goals, ← goes to the harnesses, on the focused one
  key('ArrowLeft');
  expect(document.activeElement).toBe(side()[0]);
  // ↓ previews the next one: its pane shows, nothing is used
  key('ArrowDown');
  expect(s.focused()).toBe('claude');
  expect(document.activeElement).toBe(side()[1]);
  expect(s.used).toEqual([]);
  // ↑ from the first goes round to the last
  key('ArrowUp');
  key('ArrowUp');
  expect(s.focused()).toBe('cursor');
  key('ArrowDown');
  key('ArrowDown');
  // Enter uses it and goes across to its models, on the picked one
  key('Enter');
  expect(s.used).toEqual(['claude']);
  await Promise.resolve();
  expect((document.activeElement as HTMLElement).textContent).toContain('Opus 5.5');
  // ↓↑ in the list, Enter picks (a button)
  key('ArrowUp');
  expect((document.activeElement as HTMLElement).textContent).toContain('Default');
  key('Escape');
  expect(s.closed()).toBe(1);
});

test('a_click_outside_closes_and_the_card_goes_under_the_pill_inside_the_launcher_else_above', () => {
  const s = setup();
  document.body.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true }));
  expect(s.closed()).toBe(1);
  // 1440 wide: 580 wide, its right edge on the launcher's, under the pill
  expect(pickerPlace({ left: 708, top: 294, bottom: 322 }, { right: 1302, width: 760 }, { w: 1440, h: 900 })).toEqual({ x: 708, w: 580, top: 328 });
  expect(pickerPlace({ left: 900, top: 294, bottom: 322 }, { right: 1302, width: 760 }, { w: 1440, h: 900 }).x).toBe(722);
  // a phone: the window less 12px a side; no room under it: above
  expect(pickerPlace({ left: 130, top: 180, bottom: 212 }, undefined, { w: 390, h: 844 })).toEqual({ x: 12, w: 366, top: 218 });
  expect(pickerPlace({ left: 130, top: 700, bottom: 730 }, undefined, { w: 390, h: 844 })).toEqual({ x: 12, w: 366, bottom: 150 });
});

test('a_refresh_rebuilding_the_rows_and_the_pane_keeps_each_node_and_the_focus_in_place', async () => {
  const s = setup();
  await Promise.resolve();
  const goal = document.activeElement;
  const codex = document.querySelectorAll('[data-kit="pk-h"]')[2];
  expect((goal as HTMLElement).dataset['kit']).toBe('pk-m');
  // a harness settles (ADR-0066) and the pane's data comes again as new objects: nothing is redrawn, nothing loses focus
  s.refresh();
  expect(document.activeElement).toBe(goal);
  expect(document.querySelectorAll('[data-kit="pk-h"]')[2]).toBe(codex);
  expect(codex?.querySelector('small')?.textContent).toBe('2 models');
});
