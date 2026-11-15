// The harness picker (app/HarnessPicker.tsx, v1 runner-picker.js): both panes drawn from props, keys between them.
import { expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { mount } from './helpers.tsx';
import { HarnessPicker, pickerPlace, type HarnessPickerPane } from '../src/app/HarnessPicker.tsx';
import { Launcher, type LauncherProps } from '../src/app/Home.tsx';

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

test('only_the_account_row_is_a_wrap_group_the_other_segment_rows_stay_one_line_v1_pk_acct', () => {
  setup();
  (document.querySelectorAll('[data-kit="pk-h"]')[1] as HTMLElement).click();
  const box = document.querySelector('[data-kit="picker"]') as HTMLElement;
  const wraps = [...box.querySelectorAll('[data-kit="pk-seg"][data-part$="wrap"]')];
  // v1 launcher.css `.pk-acct .pk-seg { flex-wrap: wrap }`: the accounts' segment, and no other
  expect(wraps.map((w) => w.getAttribute('data-part'))).toEqual(['runner.account-wrap']);
  expect(wraps[0]?.closest('[role="radiogroup"]')?.getAttribute('aria-label')).toBe('Account');
  expect([...(wraps[0]?.querySelectorAll('button') ?? [])].map((b) => b.textContent)).toEqual(['Auto', 'Work', 'Old']);
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

test('what_s_different_a_row_of_segments_per_setting_under_the_pane_a_press_commits_in_place_v1_pk_vary', () => {
  const anchor = document.createElement('button');
  document.body.append(anchor);
  const picked: string[] = [];
  const [sub, setSub] = createSignal('');
  let closed = 0;
  mount(() => (
    <HarnessPicker
      anchor={() => anchor}
      label="Harness and model"
      head="Harness"
      rows={ROWS.slice(1)}
      inUse="claude"
      inUseLabel="In use"
      focused="claude"
      onPreview={() => undefined}
      onUse={() => undefined}
      pane={{
        label: 'Model',
        groups: [{ items: [{ id: '', label: 'Default' }] }],
        value: '',
        onPick: () => undefined,
        empty: 'No models match',
        vary: {
          label: 'What’s different',
          rows: [
            { id: 'subagentModel', label: 'Subagents', options: [{ id: '', label: 'Task’s model' }, { id: 'haiku', label: 'Haiku', title: 'Subagents run on Haiku' }, { id: 'sonnet', label: 'Sonnet' }], value: sub(), onPick: (o) => (setSub(o), picked.push(`sub:${o}`)) },
            { id: 'skills', label: 'Skills', options: [{ id: 'on', label: 'On' }, { id: 'off', label: 'Off' }], value: 'on', onPick: (o) => picked.push(`skills:${o}`) },
          ],
        },
      }}
      onClose={() => closed++}
    />
  ));
  const box = document.querySelector('[data-part="runner.vary"]') as HTMLElement;
  expect(box.getAttribute('role')).toBe('group');
  expect(box.querySelector('[data-kit="pk-head"]')?.textContent).toBe('What’s different');
  const rows = [...box.querySelectorAll('[role="radiogroup"]')];
  expect(rows.map((r) => r.getAttribute('aria-label'))).toEqual(['Subagents', 'Skills']);
  const segs = (i: number): HTMLButtonElement[] => [...(rows[i]?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
  expect(segs(0).map((b) => [b.textContent, b.getAttribute('aria-checked')])).toEqual([
    ['Task’s model', 'true'],
    ['Haiku', 'false'],
    ['Sonnet', 'false'],
  ]);
  expect(segs(0)[1]?.title).toBe('Subagents run on Haiku');
  // a press commits in place: checked at once, the picker stays open, the same nodes
  const haiku = segs(0)[1];
  haiku?.click();
  segs(1)[1]?.click();
  expect(picked).toEqual(['sub:haiku', 'skills:off']);
  expect(segs(0)[1]).toBe(haiku);
  expect(haiku?.getAttribute('aria-checked')).toBe('true');
  expect(closed).toBe(0);
});

const launcher = (runner: Partial<LauncherProps['runner']>): LauncherProps => ({
  inLabel: 'in',
  chips: [],
  placeholder: 'What should I do?',
  value: '',
  onInput: () => undefined,
  onSend: () => undefined,
  mode: { label: 'Full auto', title: 'Mode' },
  runner: { label: 'Auto', title: 'Harness, model and thinking level', auto: true, pick: 'Claude Code', pickTitle: 'Auto will run Claude Code', compare: 'Compare', ...runner },
  lead: [],
  mid: [],
  tail: [],
  sendLabel: 'Send',
  recipes: { label: 'Recipes', empty: '', add: 'New', issue: 'Issue', issueTitle: 'Issue' },
});

test('auto_s_pick_is_the_pill_s_own_segment_after_auto_its_press_opens_what_will_auto_run_not_the_picker_v1_autoPv', () => {
  const pressed: string[] = [];
  const [open, setOpen] = createSignal(false);
  const [auto, setAuto] = createSignal(true);
  mount(() => (
    <Launcher
      {...launcher({})}
      runner={{
        ...launcher({}).runner,
        auto: auto(),
        act: { onClick: () => pressed.push('picker') },
        pickAct: { onClick: () => (pressed.push('auto-pop'), setOpen(!open())) },
        pickOpen: open(),
      }}
    />
  ));
  const pill = document.querySelector('[data-kit="runners"]') as HTMLElement;
  // one pill: "Auto" (the harness picker), the pick (what will Auto run), the + — no button beside it
  const harness = pill.querySelector<HTMLButtonElement>('[data-part="launcher.harness"]');
  const pick = pill.querySelector<HTMLButtonElement>(':scope > [data-part="launcher.autoPreview"]');
  expect(pick?.tagName).toBe('BUTTON');
  expect(harness?.contains(pick ?? null)).toBe(false);
  expect(pick?.getAttribute('aria-haspopup')).toBe('dialog');
  expect(pick?.getAttribute('aria-label')).toBe('Auto will run Claude Code');
  expect(pick?.textContent).toBe('Claude Code');
  pick?.click();
  expect(pressed).toEqual(['auto-pop']);
  expect(pick?.getAttribute('aria-expanded')).toBe('true');
  expect(pill.hasAttribute('data-open')).toBe(true);
  harness?.click();
  expect(pressed).toEqual(['auto-pop', 'picker']);
  // a harness of your own: no Auto pick; its model rides in the harness's button (v1 .rb-m), one press
  setAuto(false);
  expect(pill.querySelector('[data-part="launcher.autoPreview"]')).toBeNull();
  expect(pill.querySelector('[data-part="launcher.harness"] [data-part="launcher.model"]')?.textContent).toBe('Claude Code');
});

test('a_compare_run_s_button_opens_its_picker_and_says_its_own_model_lit_while_open', () => {
  const pressed: number[] = [];
  const [open, setOpen] = createSignal(false);
  mount(() => (
    <Launcher
      {...launcher({})}
      runner={{
        ...launcher({}).runner,
        compares: [{ label: 'Codex', removeLabel: 'Remove Codex', onRemove: () => undefined, model: 'GPT-5', act: { onClick: () => (pressed.push(0), setOpen(true)) }, open: open() }],
      }}
    />
  ));
  const run = document.querySelector<HTMLButtonElement>('[data-part="launcher.compare.runner"]');
  expect(run?.getAttribute('aria-haspopup')).toBe('dialog');
  expect(run?.querySelector('[data-part="launcher.compare.model"]')?.textContent).toBe('GPT-5');
  expect(run?.title).toBe('Codex · GPT-5');
  run?.click();
  expect(pressed).toEqual([0]);
  expect(run?.getAttribute('aria-expanded')).toBe('true');
  expect(document.querySelector('[data-kit="runners"]')?.hasAttribute('data-open')).toBe(true);
});

test('a_hand_picked_harness_s_pill_marks_the_default_model_and_splits_model_effort_account_for_the_narrow_rules_v1_rb_m_def', () => {
  const [def, setDef] = createSignal(true);
  mount(() => (
    <Launcher
      {...launcher({})}
      runner={{
        ...launcher({}).runner,
        auto: false,
        label: 'Claude Code',
        pick: def() ? 'Default' : 'Opus 4.7 · High · work',
        pickParts: def() ? { model: 'Default', def: true, effort: '', account: '' } : { model: 'Opus 4.7', def: false, effort: 'High', account: 'work' },
      }}
    />
  ));
  const btn = document.querySelector<HTMLElement>('[data-part="launcher.harness"]');
  const model = (): HTMLElement | null | undefined => btn?.querySelector('[data-part="launcher.model"]');
  // the default model: app-home.css ≤520px keys on data-def to keep "Claude Code" and fold "Default" (v1 .rb-m.def)
  expect(btn?.hasAttribute('data-def')).toBe(true);
  expect(btn?.querySelector('[data-kit="rb-h"]')?.textContent).toBe('Claude Code');
  expect(model()?.querySelector('[data-kit="rb-m"]')?.textContent).toBe('Default');
  expect(model()?.querySelector('[data-kit="rb-e"]')).toBeNull();
  // a model of your own: no data-def (≤520px the harness name goes, the model stays); the effort (≤520px) and the
  // account (≤720px) are their own parts, the words the same as the joined pick
  setDef(false);
  expect(btn?.hasAttribute('data-def')).toBe(false);
  expect(model()?.querySelector('[data-kit="rb-m"]')?.textContent).toBe('Opus 4.7');
  expect(model()?.querySelector('[data-kit="rb-e"]')?.textContent).toBe(' · High');
  expect(model()?.querySelector('[data-kit="rb-a"]')?.textContent).toBe(' · work');
  expect(model()?.textContent).toBe('Opus 4.7 · High · work');
});

test('auto_never_marks_a_default_model_on_the_harness_button', () => {
  mount(() => <Launcher {...launcher({})} runner={{ ...launcher({}).runner, pickParts: { model: 'Default', def: true, effort: '', account: '' } }} />);
  expect(document.querySelector('[data-part="launcher.harness"]')?.hasAttribute('data-def')).toBe(false);
});
