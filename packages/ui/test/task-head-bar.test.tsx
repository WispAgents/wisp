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

// owner 2026-11-28: the wide column reads as one sentence: the harness, 📁 in wisp, on This computer, the branch, then
// the usage (it was harness, project, usage, then computer and branch)
test('the_details_read_harness_in_project_on_computer_then_branch_then_usage', () => {
  const host = mount(() => (
    <TaskDetails
      harness="Claude Code · Opus"
      project="wisp"
      color="blue"
      machine={{ name: 'This computer', icon: 'laptop' }}
      branch={{ head: 'wisp/task', tail: '-33bfeb', title: 'wisp/task-33bfeb' }}
      usage={{ context: { text: '8% context', title: '8%', share: 0.08, hi: false }, sections: () => [], label: 'Task usage', more: { label: 'Usage', onClick: () => {} } }}
      leads={{ project: 'in', machine: 'on' }}
      actions={[]}
    />
  ));
  const bar = host.querySelector('[data-kit="it-bar"]');
  const order = [...(bar?.querySelectorAll('[data-kit="ml-name"], [data-kit="q-proj"], [data-kit="m-where"], [data-kit="m-branch"], [data-kit="plan-used"]') ?? [])].map((e) => (e as HTMLElement).dataset['kit']);
  expect(order).toEqual(['ml-name', 'q-proj', 'm-where', 'm-branch', 'plan-used']);
  const inProject = host.querySelector('[data-kit="it-in"]');
  expect(inProject?.querySelector('[data-kit="icon"]')).not.toBeNull();
  expect(inProject?.textContent).toBe('inwisp');
  expect(host.querySelector('[data-kit="m-where"]')?.textContent).toBe('onThis computer');
});

test('the_details_without_leads_add_no_words_or_folder_icon', () => {
  const host = mount(() => <TaskDetails harness="Claude Code · Opus" project="wisp" color="blue" machine={{ name: 'This computer', icon: 'laptop' }} actions={[]} />);
  expect(host.querySelector('[data-kit="it-lead"]')).toBeNull();
  expect(host.querySelector('[data-kit="it-in"] > [data-kit="icon"]')).toBeNull();
  expect(host.querySelector('[data-kit="m-where"]')?.textContent).toBe('This computer');
});
