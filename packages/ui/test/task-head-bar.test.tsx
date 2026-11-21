// docs/ux/2026-11-18-desktop.md #4: the task's actions are one bar in the head (Changes +n −n, Ship ▾, Done, ⋯ before
// the panel toggle and ×), the same with and without the side panel; the details column keeps only facts.
import { expect, test } from 'vitest';
import { TaskDetails, TaskHead, type HeadAction } from '../src/index.ts';
import { mount } from './helpers.tsx';

const head = { title: 'Fix the login', color: 'blue', renameLabel: 'Rename', panelLabel: 'Panel', closeLabel: 'Close' } as const;
const bar: HeadAction[] = [
  { id: 'changes', key: 'D', icon: 'file', label: 'Changes', title: 'Changes (D)', figures: { plus: '+12', minus: '−3' }, act: { onClick: () => {} } },
  { id: 'ship', key: 'S', icon: 'send', label: 'Ship', title: 'Ship (S)', act: { menu: { items: [{ value: 'ship', label: 'Ship it…' }], onSelect: () => {} } } },
  { id: 'done', key: 'E', icon: 'done', label: 'Done', title: 'Mark done (E)', tone: 'good', act: { onClick: () => {} } },
  { id: 'more', icon: 'dots', label: 'More', title: 'More', bare: true, act: { onClick: () => {} } },
];

test('the_head_holds_the_actions_in_one_bar_before_panel_and_close_ux_4', () => {
  for (const panelOn of [false, true]) {
    const host = mount(() => <TaskHead {...head} bar={bar} panelOn={panelOn} />);
    const acts = [...host.querySelectorAll('[data-kit="sh-acts"] > button')].map((b) => (b as HTMLElement).dataset['a'] ?? b.getAttribute('aria-label'));
    expect(acts).toEqual(['changes', 'ship', 'done', 'more', 'Panel', 'Close']);
    const changes = host.querySelector('[data-a="changes"]');
    expect(changes?.textContent).toContain('+12');
    expect(changes?.textContent).toContain('−3');
    // keys in the tooltip, never a keycap; ⋯ says its words only to the screen reader and tooltip
    expect(host.querySelector('[data-a="done"]')?.getAttribute('title')).toBe('Mark done (E)');
    expect(host.querySelector('[data-a="more"] [data-text="control"]')).toBeNull();
    expect(host.querySelector('[data-a="more"]')?.getAttribute('aria-label')).toBe('More');
  }
});

test('the_details_keep_only_facts_no_actions_row_ux_4', () => {
  const host = mount(() => <TaskDetails harness="Claude Code · Opus" project="wisp" color="blue" actions={[]} />);
  expect(host.querySelector('[data-kit="it-actions"]')).toBeNull();
  expect(host.querySelector('[data-kit="ml-name"]')?.textContent).toBe('Claude Code · Opus');
});
