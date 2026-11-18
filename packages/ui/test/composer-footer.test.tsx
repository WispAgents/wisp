// The composer's one footer (docs/ux/2026-11-18-desktop.md #3): a feature that's on is a chip with ✕ after the Where chip,
// and Compare lives in `+` (the pill's own + goes); the kit's parts are the same parts (no spell breaks).
import { afterEach, expect, test, vi } from 'vitest';
import { mount, part } from './helpers.tsx';
import { Launcher, type LauncherProps } from '../src/app/Home.tsx';
import { ReplyBox } from '../src/app/Task.tsx';

afterEach(() => {
  document.body.innerHTML = '';
});

const base = (o: Partial<LauncherProps> = {}): LauncherProps => ({
  inLabel: 'in',
  place: { label: 'wisp', colors: ['blue'], onOpen: () => undefined },
  placeholder: 'What should I go do?',
  value: '',
  onInput: () => undefined,
  onSend: () => undefined,
  mode: { label: 'Full auto', title: 'Mode' },
  runner: { label: 'Claude Code', title: 'Harness', auto: false, pick: '', pickTitle: '', compare: 'Compare', compareAct: { onClick: () => undefined } },
  lead: [],
  mid: [],
  tail: [],
  sendLabel: 'Send',
  recipes: { label: 'Recipes', empty: '', add: 'New', issue: 'Issue', issueTitle: 'Issue' },
  ...o,
});

test('a feature that is on is a chip with its own clear, its press still the tool’s, its part unchanged', () => {
  const opened = vi.fn();
  const cleared = vi.fn();
  mount(() => (
    <Launcher
      {...base({
        tail: [{ id: 'goal', part: 'launcher.goal', icon: 'target', label: 'Goal: tests pass', title: 'Goal', text: true, on: true, act: { onClick: opened }, clear: { label: 'Turn Goal off', act: { onClick: cleared } } }],
      })}
    />
  ));
  const chip = document.querySelector('[data-kit="l-bar"] > [data-kit="l-chip"]');
  expect(chip?.getAttribute('data-a'), 'measured as the tool').toBe('goal');
  const press = part('launcher.goal') as HTMLButtonElement;
  expect(press.textContent).toContain('Goal: tests pass');
  press.click();
  expect(opened).toHaveBeenCalledOnce();
  const x = chip?.querySelector<HTMLButtonElement>('[data-kit="chip-x"]');
  expect(x?.getAttribute('aria-label')).toBe('Turn Goal off');
  x?.click();
  expect(cleared).toHaveBeenCalledOnce();
});

test('compare in the menu: the pill’s + goes, a compare run keeps its ×', () => {
  mount(() => <Launcher {...base({ runner: { ...base().runner, compareInMenu: true, compares: [{ label: 'Codex', removeLabel: 'Remove Codex', onRemove: () => undefined, act: {} }] } })} />);
  expect((part('launcher.compare') as HTMLButtonElement | null)?.hidden, 'not drawn (as when the runs are full)').toBe(true);
  expect(part('launcher.compare.remove')).not.toBeNull();
});

// the reply box: the same footer, without the project picker. `+` holds attach, Goal, Delegate and Start later; a
// feature that's on is a chip; the harness says its model beside its logo, not a bare logo
test('the reply box with plus: one + for what is off, chips for what is on, the harness with its model', () => {
  const cleared = vi.fn();
  mount(() => (
    <ReplyBox
      placeholder="Follow up"
      value=""
      onInput={() => undefined}
      onSend={() => undefined}
      attach="Attach"
      mode={{ label: 'Full auto', title: 'Mode' }}
      runner={{ title: 'Claude Code · opus', model: 'opus' }}
      team="Delegate"
      goal="Goal"
      later="Send later"
      send="Send"
      plus={{ label: 'Add', title: 'Attach files, a goal, delegates, send later', act: { menu: { items: [{ value: 'attach', label: 'Attach files', part: 'composer.attach' }], onSelect: () => undefined } } }}
      chips={[{ id: 'goal', part: 'composer.goal', icon: 'target', label: 'Goal: tests pass', title: 'Goal', text: true, on: true, act: {}, clear: { label: 'Turn Goal off', act: { onClick: cleared } } }]}
    />
  ));
  const bar = document.querySelector('[data-kit="c-bar"]') as HTMLElement;
  expect(part('composer.plus')?.getAttribute('aria-label')).toBe('Add');
  // what's off is in `+`, not on the bar
  for (const p of ['composer.attach', 'composer.team', 'composer.sendLater']) expect(bar.querySelector(`[data-part="${p}"]`), p).toBeNull();
  // what's on: a chip, cleared in one press
  expect(bar.querySelector('[data-part="composer.goal"]')?.textContent).toContain('Goal: tests pass');
  bar.querySelector<HTMLButtonElement>('[data-part="composer.goal.clear"]')?.click();
  expect(cleared).toHaveBeenCalledOnce();
  // the harness: its logo and the model it runs on
  expect(part('composer.harness')?.textContent).toContain('opus');
});
