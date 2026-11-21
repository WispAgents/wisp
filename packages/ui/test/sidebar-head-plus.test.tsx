// docs/ux/2026-11-18-desktop.md #11: the bare + beside "All ▾" read as "new task" but meant Manage projects. The head
// draws it only when it's given something to do (its act or onManage); the shell gives none: the switcher has them.
import { expect, test } from 'vitest';
import { SidebarHead } from '../src/index.ts';
import { mount } from './helpers.tsx';

const base = { brandLabel: 'wisp', searchLabel: 'Search', accountLabel: 'You', viewLabel: 'Hide', manageLabel: 'Manage projects', chips: [], on: 'all' } as const;

test('the_head_draws_no_bare_plus_unless_given_one', () => {
  const host = mount(() => <SidebarHead {...base} />);
  expect(host.querySelector('[data-kit="p-chip"][data-add]')).toBeNull();
  const given = mount(() => <SidebarHead {...base} onManage={() => {}} />);
  expect(given.querySelector('[data-kit="p-chip"][data-add]')).not.toBeNull();
});
