// The harness picker (app/HarnessPicker.tsx, v1 runner-picker.js): both panes drawn from props, keys between them.
import { expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { mount } from './helpers.tsx';
import { PlatformLookProvider } from '../src/platform/PlatformLook.tsx';
import { HarnessPicker, pickerPlace, type HarnessPickerPane } from '../src/app/HarnessPicker.tsx';
import { Launcher, type LauncherProps } from '../src/app/Home.tsx';
import HOME_CSS from '../src/css/app-home.css?raw';

const ROWS = [
  { id: 'auto', name: 'Auto', auto: true, sub: 'Picks per task' },
  { id: 'claude', name: 'Claude Code', logo: '/claude.svg', sub: '4 models' },
  { id: 'codex', name: 'Codex', logo: '/codex.svg', sub: 'Checking…' },
  { id: 'cursor', name: 'Cursor', sub: 'Not installed', off: true },
];

/** the pane draws the frame after the card (audit H tasks NEW #3) */
const frame = (): Promise<void> => new Promise((r) => requestAnimationFrame(() => queueMicrotask(r)));

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
  await frame();
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

test('a_harness_shows_its_models_thinking_and_accounts_on_the_right', async () => {
  const s = setup();
  await frame();
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

test('only_the_account_row_is_a_wrap_group_the_other_segment_rows_stay_one_line_v1_pk_acct', async () => {
  setup();
  await frame();
  (document.querySelectorAll('[data-kit="pk-h"]')[1] as HTMLElement).click();
  const box = document.querySelector('[data-kit="picker"]') as HTMLElement;
  const wraps = [...box.querySelectorAll('[data-kit="pk-seg"][data-part$="wrap"]')];
  // v1 launcher.css `.pk-acct .pk-seg { flex-wrap: wrap }`: the accounts' segment, and no other
  expect(wraps.map((w) => w.getAttribute('data-part'))).toEqual(['runner.account-wrap']);
  // it wraps as a sanctioned Wrap group (layout.css `[data-layout='wrap']`), and it is the only pk-seg that does
  expect([...box.querySelectorAll('[data-kit="pk-seg"][data-layout="wrap"]')]).toEqual(wraps);
  expect(wraps[0]?.closest('[role="radiogroup"]')?.getAttribute('aria-label')).toBe('Account');
  expect([...(wraps[0]?.querySelectorAll('button') ?? [])].map((b) => b.textContent)).toEqual(['Auto', 'Work', 'Old']);
});

test('keys_arrows_preview_in_the_left_pane_right_and_enter_go_across_left_comes_back_escape_closes', async () => {
  const s = setup();
  await frame();
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
  await frame();
  const goal = document.activeElement;
  const codex = document.querySelectorAll('[data-kit="pk-h"]')[2];
  expect((goal as HTMLElement).dataset['kit']).toBe('pk-m');
  // a harness settles (ADR-0066) and the pane's data comes again as new objects: nothing is redrawn, nothing loses focus
  s.refresh();
  expect(document.activeElement).toBe(goal);
  expect(document.querySelectorAll('[data-kit="pk-h"]')[2]).toBe(codex);
  expect(codex?.querySelector('small')?.textContent).toBe('2 models');
});

test('what_s_different_a_row_of_segments_per_setting_under_the_pane_a_press_commits_in_place_v1_pk_vary', async () => {
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
  await frame();
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
  place: { label: 'wisp', colors: ['blue'], onOpen: () => undefined },
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

test('comparing_the_lead_keeps_its_model_in_its_button_and_try_it_two_ways_marks_the_pill_v1_runners_cmp_rb_m', () => {
  const [vary, setVary] = createSignal(false);
  mount(() => (
    <Launcher
      {...launcher({})}
      runner={{
        ...launcher({}).runner,
        auto: false,
        label: 'Claude Code',
        pick: 'Opus 4.7 · High · work',
        pickParts: { model: 'Opus 4.7', def: false, effort: 'High', account: 'work' },
        compares: [{ label: 'Claude Code', removeLabel: 'Remove', onRemove: () => undefined, act: { onClick: () => undefined }, ...(vary() && { variant: ['Haiku subagents'] }) }],
      }}
    />
  ));
  const pill = document.querySelector<HTMLElement>('[data-kit="runners"]');
  // v1 .runners.cmp: the lead's .rb-m stays (only ≤640px folds it); .rb-a goes, .rb-e goes unless it's a trial
  const lead = pill?.querySelector('[data-part="launcher.harness"] [data-part="launcher.model"]');
  expect(lead?.querySelector('[data-kit="rb-m"]')?.textContent).toBe('Opus 4.7');
  expect(pill?.hasAttribute('data-cmp')).toBe(true);
  expect(pill?.hasAttribute('data-trial')).toBe(false);
  setVary(true);
  expect(pill?.hasAttribute('data-trial')).toBe(true);
});

test('try_it_two_ways_folds_the_lead_as_v1_runners_trial_its_name_and_default_under_1099px_its_thinking_under_700px', () => {
  mount(() => (
    <Launcher
      {...launcher({})}
      runner={{
        ...launcher({}).runner,
        auto: false,
        label: 'Claude Code',
        pick: 'Default · High',
        pickParts: { model: 'Default', def: true, effort: 'High', account: '' },
        compares: [{ label: 'Claude Code', removeLabel: 'Remove', onRemove: () => undefined, act: { onClick: () => undefined }, variant: ['Subagents on Haiku'] }],
      }}
    />
  ));
  const lead = document.querySelector<HTMLElement>('[data-kit="runners"][data-trial] > [data-part="launcher.harness"][data-def]');
  // the bar folds with it (v1 .l-bar.l-try): Delegate's words at 800px, the mode's and Goal's at 780px
  expect(document.querySelector('[data-kit="l-bar"]')?.hasAttribute('data-try')).toBe(true);
  // the thinking level's " · " is its own node, so folding "Default" leaves "High", not " · High" (v1 .rb-e)
  const effort = lead?.querySelector('[data-kit="rb-e"]');
  expect(effort?.textContent).toBe(' · High');
  expect(effort?.querySelector('[data-kit="rb-sep"]')?.textContent).toBe(' · ');
  // v1 launcher.css: @container launcher (max-width: 1099px) .runners.trial .rb-h / .rb-m.def; 641–1099 .rb-h back
  // beside a default model; (max-width: 700px) .rb-e; (max-width: 640px) the lead's chevron
  const rules = HOME_CSS.replace(/\s+/g, ' ');
  expect(rules).toContain("@container launcher (max-width: 1099px) { [data-kit='runners'][data-trial] > [data-kit='runner-btn'][data-lead] > [data-kit='rb-h'], [data-kit='runners'][data-trial] > [data-kit='runner-btn'][data-def] [data-kit='rb-m'], [data-kit='runners'][data-trial] > [data-kit='runner-btn'][data-def] [data-kit='rb-sep'] { display: none; }");
  expect(rules).toContain("@container launcher (not (max-width: 640px)) and (max-width: 1099px) { [data-kit='runners'][data-trial] > [data-kit='runner-btn'][data-def] > [data-kit='rb-h'] { display: block; }");
  expect(rules).toContain("@container launcher (max-width: 700px) { [data-kit='runners'][data-trial] [data-kit='rb-e'] { display: none; } }");
  expect(rules).toContain("[data-kit='runners'][data-trial] > [data-kit='runner-btn'][data-lead] > [data-kit='icon'] { display: none; }");
  expect(rules).toContain("@container launcher (max-width: 800px) { [data-kit='l-bar'][data-try] > [data-a='team'] > [data-text='control'] { display: none; } }");
  expect(rules).toContain("@container launcher (max-width: 780px) { [data-kit='l-bar'][data-try] > [data-kit='mode-btn'] > [data-text='control'], [data-kit='l-bar'][data-try] > [data-a='goal'] > [data-text='control'] { display: none; } }");
});

test('a_compare_run_on_its_harness_s_default_model_says_default_after_its_name_v1_rb_m_def', () => {
  mount(() => (
    <Launcher
      {...launcher({})}
      runner={{ ...launcher({}).runner, compares: [{ label: 'Codex', removeLabel: 'Remove Codex', onRemove: () => undefined, model: 'Default', act: { onClick: () => undefined } }] }}
    />
  ));
  const run = document.querySelector<HTMLElement>('[data-part="launcher.compare.runner"]');
  expect(run?.querySelector('[data-part="launcher.compare.model"]')?.textContent).toBe('Default');
  expect(document.querySelector('[data-kit="runners"]')?.hasAttribute('data-trial')).toBe(false);
  expect(document.querySelector('[data-kit="l-bar"]')?.hasAttribute('data-try')).toBe(false);
  // "Default" gives way before the run's name, as v1 .rb-m (flex 0 10000 auto)
  expect(HOME_CSS.replace(/\s+/g, ' ')).toContain("[data-kit='runner-btn'][data-cmp] > [data-part~='launcher.compare.model'] { flex-shrink: 10000; margin-left: 13px; padding-left: 0; color: var(--muted); }");
});

test('on_a_phone_with_no_recipes_start_from_an_issue_is_a_tool_in_the_bar_not_alone_on_a_row_v1', () => {
  // owner 2026-11-17: the phone sheet's "Start from an issue" chip sat alone on a row of its own under Send
  mount(() => (
    <PlatformLookProvider look={() => 'ios'}>
      <Launcher {...launcher({})} />
    </PlatformLookProvider>
  ));
  const issue = document.querySelector('[data-part~="launcher.issue"]');
  expect(issue?.closest('[data-kit="l-bar"]')).not.toBeNull();
  expect(document.querySelector('[data-kit="l-recipes"]')?.hasAttribute('hidden')).toBe(true);
  expect(document.querySelectorAll('[data-part~="launcher.issue"]')).toHaveLength(1);
});

test('in_the_browser_start_from_an_issue_stays_at_the_end_of_the_recipes_row_v1', () => {
  mount(() => <Launcher {...launcher({})} />);
  const issue = document.querySelector('[data-part~="launcher.issue"]');
  expect(issue?.closest('[data-kit="l-recipes"]')).not.toBeNull();
  expect(document.querySelector('[data-kit="l-recipes"]')?.hasAttribute('hidden')).toBe(false);
});

test('in_the_phone_apps_with_recipes_in_more_neither_their_row_nor_an_issue_tool_shows_qa_r2_2', () => {
  mount(() => (
    <PlatformLookProvider look={() => 'android'}>
      <Launcher {...launcher({})} recipes={{ ...launcher({}).recipes, inMore: true }} />
    </PlatformLookProvider>
  ));
  expect(document.querySelector('[data-kit="l-recipes"]')?.hasAttribute('hidden')).toBe(true);
  expect(document.querySelector('[data-kit="l-bar"] [data-part~="launcher.issue"]')).toBeNull();
});

test('nothing_to_send_the_round_send_is_off_like_the_sheets_header_send_qa_r2_12', () => {
  const [off, setOff] = createSignal(true);
  mount(() => <Launcher {...launcher({})} sendOff={off()} />);
  const send = document.querySelector('[data-part="launcher.send"]') as HTMLButtonElement;
  expect(send.disabled).toBe(true);
  setOff(false);
  expect(send.disabled).toBe(false);
});

// UX 2026-11-18 #9: the list holds the harnesses that can run; the way to another is a row at its bottom
test('the_harness_list_ends_with_set_up_another_agent_ux_2026_11_18_9', () => {
  let opened = 0;
  const anchor = document.createElement('button');
  document.body.append(anchor);
  mount(() => (
    <HarnessPicker
      anchor={() => anchor}
      label="Harness and model"
      head="Harness"
      rows={[{ id: 'claude', name: 'Claude Code', sub: '' }]}
      inUse="claude"
      inUseLabel="In use"
      focused="claude"
      onPreview={() => undefined}
      onUse={() => undefined}
      pane={{ label: 'Model', groups: [{ items: [{ id: '', label: 'Default' }] }], value: '', onPick: () => undefined, empty: 'None' }}
      onClose={() => undefined}
      setup={{ label: 'Set up another agent…', onClick: () => opened++ }}
    />
  ));
  const side = document.querySelector('[data-kit="pk-side"]') as HTMLElement;
  const setup = side.querySelector<HTMLButtonElement>('[data-part="runner.picker.setup"]');
  expect(setup?.textContent).toBe('Set up another agent…');
  // last in the list, after every harness
  expect(side.lastElementChild).toBe(setup);
  setup?.click();
  expect(opened).toBe(1);
  // no second line where there's nothing to say
  expect(side.querySelector('[data-kit="pk-h"] small')).toBeNull();
});

test('the_card_and_its_harnesses_show_in_the_press_frame_and_the_pane_the_frame_after_audit_h_tasks_new3', async () => {
  // WebKit took 102–130 ms to show anything after the pill's press (ADR-0017: within 100): both panes were built in it.
  // Now the card, its harnesses and the focused row come at once; the models, thinking and accounts a frame later.
  setup();
  const box = document.querySelector('[data-kit="picker"]') as HTMLElement;
  expect(box.querySelectorAll('[data-kit="pk-h"]').length).toBe(4);
  expect(box.querySelector('[data-kit="pk-main"]')?.hasAttribute('data-drawn')).toBe(false);
  expect(box.querySelectorAll('[data-kit="pk-m"]').length).toBe(0);
  await Promise.resolve();
  expect((document.activeElement as HTMLElement).dataset['kit']).toBe('pk-h'); // the keys at once, on the focused row
  await frame();
  expect(box.querySelector('[data-kit="pk-main"]')?.hasAttribute('data-drawn')).toBe(true);
  expect(box.querySelectorAll('[data-kit="pk-m"]').length).toBe(3);
  expect((document.activeElement as HTMLElement).dataset['kit']).toBe('pk-m'); // then the pick, as before
});

// Audit H tasks notes: a picked formula or task file's agent (critic = codex gpt-5 high) read "gpt-5 · high" on a
// phone-width browser (who runs folded away) and "critic · Codex" alone in the phone apps (desktop says both).
test('a_picked_files_pill_names_its_agent_on_a_narrow_bar_and_its_model_in_the_phone_apps_audit_h_tasks', async () => {
  mount(() => <Launcher {...launcher({})} runner={{ ...launcher({}).runner, auto: false, file: true, label: 'critic · Codex', pick: 'gpt-5 · high' }} />);
  const pill = document.querySelector<HTMLElement>('[data-part="launcher.harness"]');
  expect(pill?.hasAttribute('data-file')).toBe(true);
  expect(pill?.querySelector('[data-kit="rb-h"]')?.textContent?.trim()).toBe('critic · Codex');
  expect(pill?.querySelector('[data-kit="apv-l"]')?.textContent?.trim()).toBe('gpt-5 · high');
  const css = HOME_CSS.replace(/\/\*[^]*?\*\//g, '');
  const narrow = css.slice(css.indexOf('@container launcher (max-width: 520px)'));
  expect(narrow).toMatch(/\[data-kit='runner-btn'\]\[data-file\] > \[data-kit='rb-h'\] \{\s*display: block;/);
  expect(narrow).toMatch(/\[data-kit='runner-btn'\]\[data-file\] > \[data-kit='auto-pv'\] \{\s*display: none;/);
  // and the measured bar's last squeeze, its other narrow form
  expect(css).toMatch(/:where\(\[data-kit='launcher'\]\[data-squeeze='3'\]\) \[data-kit='runner-btn'\]\[data-file\] > \[data-kit='rb-h'\] \{\s*display: block;/);
  const looks = import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true });
  for (const look of ['ios', 'android']) {
    const sheet = (Object.entries(looks).find(([p]) => p.endsWith(`/${look}.css`))?.[1] ?? '').replace(/\/\*[^]*?\*\//g, '');
    expect(sheet, look).toMatch(new RegExp(`:root\\[data-platform='${look}'\\] \\[data-kit='launcher'\\] \\[data-kit='runner-btn'\\]\\[data-file\\] > \\[data-kit='auto-pv'\\] \\{ display: flex;`));
  }
  // a harness picked by hand is unchanged
  mount(() => <Launcher {...launcher({})} runner={{ ...launcher({}).runner, auto: false, label: 'Codex', pick: 'gpt-5' }} />);
  expect(document.querySelectorAll('[data-part="launcher.harness"]')[1]?.hasAttribute('data-file')).toBe(false);
});

test('harness picker: the find box is focused on open, Tab jumps between harness groups, f stars a row, Sign in under a row', async () => {
  const anchor = document.createElement('button');
  document.body.append(anchor);
  const [q, setQ] = createSignal('');
  const starred: string[] = [];
  const picked: string[] = [];
  const signIn: string[] = [];
  const pane = (): HarnessPickerPane => ({
    label: 'Model',
    groups: q()
      ? [
          { head: 'Claude Code', items: [{ id: 'claude\topus', label: 'Opus 5.5', star: false }, { id: 'claude\tsonnet', label: 'Sonnet 5.5', star: true }] },
          { head: 'opencode', items: [{ id: 'opencode\tanthropic/opus', label: 'Opus' }] },
        ]
      : [{ items: [{ id: 'opus', label: 'Opus 5.5', star: false }] }],
    value: '',
    onPick: (id) => picked.push(id),
    empty: 'No harness or model matches',
    onStar: (id) => starred.push(id),
    starLabel: 'Favourite (f)',
  });
  mount(() => (
    <HarnessPicker
      anchor={() => anchor}
      label="Harness"
      head="Harness"
      rows={[...ROWS.slice(1, 3), { id: 'grok', name: 'Grok', sub: 'Signed out', off: true, action: { label: 'Sign in', onClick: () => signIn.push('grok') } }]}
      inUse="claude"
      inUseLabel="In use"
      focused="claude"
      onPreview={() => undefined}
      onUse={() => undefined}
      pane={pane()}
      onClose={() => undefined}
      find={{ placeholder: 'Search harnesses and models', value: q(), onInput: setQ }}
    />
  ));
  await frame();
  const find = document.querySelector<HTMLInputElement>('[data-part="runner.find"] input');
  expect(document.activeElement).toBe(find);
  setQ('opus');
  key('Tab');
  expect((document.activeElement as HTMLElement).dataset['id']).toBe('claude\topus');
  key('Tab');
  expect((document.activeElement as HTMLElement).dataset['id'], 'the next harness').toBe('opencode\tanthropic/opus');
  key('Tab');
  expect((document.activeElement as HTMLElement).dataset['id'], 'round to the first').toBe('claude\topus');
  key('f');
  expect(starred).toEqual(['claude\topus']);
  expect([...document.querySelectorAll('[data-kit="pk-star"]')].map((s) => s.getAttribute('aria-checked'))).toEqual(['false', 'true']);
  key('Enter');
  document.querySelector<HTMLButtonElement>('[data-kit="pk-hact"]')?.click();
  expect(signIn).toEqual(['grok']);
  document.querySelectorAll('[data-kit="picker"]').forEach((n) => n.remove());
  anchor.remove();
});

/** ADR-0128: the Browser row, as Cursor shows it (no per-run MCP): its choices greyed out with why, Off in effect, and
 *  the reason under them; a greyed choice can't be picked. */
test('the_browser_row_greys_out_what_the_harness_cant_take_with_why', async () => {
  const anchor = document.createElement('button');
  document.body.append(anchor);
  const picked: string[] = [];
  const why = 'Cursor takes no MCP servers from wisp, so it has no browser here';
  const pane: HarnessPickerPane = {
    label: 'Model',
    groups: [{ items: [{ id: '', label: 'Default' }] }],
    value: '',
    onPick: () => undefined,
    empty: 'No models match',
    browser: {
      label: 'Browser',
      options: [
        { id: 'default', label: 'Default', off: true, title: why },
        { id: 'off', label: 'Off' },
        { id: 'chrome', label: 'On (Chrome)', off: true, title: why },
        { id: 'headless', label: 'Headless', off: true, title: why },
      ],
      value: 'off',
      onPick: (b) => picked.push(b),
      note: why,
    },
  };
  mount(() => (
    <HarnessPicker
      anchor={() => anchor}
      label="Harness and model"
      head="Harness"
      rows={[{ id: 'cursor', name: 'Cursor', sub: '1 model' }]}
      inUse="cursor"
      inUseLabel="In use"
      focused="cursor"
      onPreview={() => undefined}
      onUse={() => undefined}
      pane={pane}
      onClose={() => undefined}
    />
  ));
  await frame();
  const row = document.querySelector('[data-part="runner.browser"]') as HTMLElement;
  expect(row.getAttribute('aria-label')).toBe('Browser');
  const segs = [...row.querySelectorAll<HTMLButtonElement>('[data-kit="pk-seg"] > button')];
  expect(segs.map((b) => [b.textContent, b.getAttribute('aria-checked'), b.getAttribute('aria-disabled')])).toEqual([
    ['Default', 'false', 'true'],
    ['Off', 'true', null],
    ['On (Chrome)', 'false', 'true'],
    ['Headless', 'false', 'true'],
  ]);
  expect(segs[2]?.title).toBe(why);
  expect(row.querySelector('[data-kit="pk-anote"]')?.textContent).toBe(why);
  segs[3]?.click();
  segs[1]?.click();
  expect(picked).toEqual(['off']);
});
