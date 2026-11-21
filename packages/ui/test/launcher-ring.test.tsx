// UX 2026-11-18 desktop #8: the launcher's harness chip draws a usage ring around its logo when the shell passes one
// (the picked harness past 80% of a plan window), its words in the chip's tooltip; none: the chip is as it was.
import { expect, test } from 'vitest';
import { mount } from './helpers.tsx';
import { Launcher, type LauncherProps } from '../src/app/Home.tsx';
import HOME_CSS from '../src/css/app-home.css?raw';

const launcher = (runner: Partial<LauncherProps['runner']>): LauncherProps => ({
  inLabel: 'in',
  place: { label: 'wisp', colors: ['blue'], onOpen: () => undefined },
  placeholder: 'What should I do?',
  value: '',
  onInput: () => undefined,
  onSend: () => undefined,
  mode: { label: 'Full auto', title: 'Mode' },
  runner: { label: 'Claude Code', title: 'Harness', auto: false, pick: 'Default', pickTitle: 'Harness', compare: 'Compare', logo: '/claude.svg', ...runner },
  lead: [],
  mid: [],
  tail: [],
  sendLabel: 'Send',
  recipes: { label: 'Recipes', empty: '', add: 'New', issue: 'Issue', issueTitle: 'Issue' },
});

test('the_harness_chip_rings_its_logo_past_80_percent_of_a_plan_window', () => {
  const host = mount(() => <Launcher {...launcher({ usage: { pct: 86, tone: 'warn', title: 'Weekly 86% used' } })} />);
  const ring = host.querySelector<HTMLElement>('[data-part~="launcher.harness"] [data-kit="rb-ring"]');
  expect(ring?.dataset['tone']).toBe('warn');
  expect(ring?.style.getPropertyValue('--ctx')).toBe('86%');
  // its words are the chip's (the tooltip and what a screen reader says), the ring itself is drawing only
  const chip = host.querySelector('[data-part~="launcher.harness"]');
  expect(ring?.getAttribute('aria-hidden')).toBe('true');
  expect(ring?.dataset['part']).toBe('launcher.harness.usage');
  expect(chip?.getAttribute('title')).toBe('Harness · Weekly 86% used');
  expect(chip?.getAttribute('aria-description')).toBe('Weekly 86% used');
  expect(HOME_CSS).toMatch(/\[data-kit='rb-ring'\][^{]*\{[^}]*conic-gradient/);
});

test('no_ring_under_80_percent', () => {
  const host = mount(() => <Launcher {...launcher({})} />);
  expect(host.querySelector('[data-kit="rb-ring"]')).toBeNull();
  expect(host.querySelector('[data-part~="launcher.harness"]')?.getAttribute('title')).toBe('Harness');
});
