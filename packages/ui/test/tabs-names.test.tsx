// QA round 1 #17: the side panel's tabs (Changes, PR, Shell, Preview) are icons alone in a narrow panel (their labels
// hidden by its container query), and had no aria-label and no title: a screen reader read nothing, a mouse got no
// hint. A tab with an icon is named by its label, and says it on hover.
import { expect, test } from 'vitest';
import { Tabs } from '../src/index.ts';
import { mount } from './helpers.tsx';

test('a_tab_with_an_icon_is_named_and_titled_by_its_label', () => {
  const host = mount(() => (
    <Tabs
      aria-label="Side panel"
      value="changes"
      onChange={() => {}}
      items={[
        { value: 'changes', label: 'Changes', icon: 'file' },
        { value: 'pr', label: 'PR', icon: 'branch', title: 'Pull request' },
        { value: 'plain', label: 'Plain' },
      ]}
    />
  ));
  const tab = (v: string): HTMLElement | null => host.querySelector(`[data-kit="tab"][data-value="${v}"]`);
  expect([tab('changes')?.getAttribute('aria-label'), tab('changes')?.getAttribute('title')]).toEqual(['Changes', 'Changes']);
  expect([tab('pr')?.getAttribute('aria-label'), tab('pr')?.getAttribute('title')]).toEqual(['PR', 'Pull request']);
  // a text tab is named by its text already
  expect([tab('plain')?.getAttribute('aria-label'), tab('plain')?.getAttribute('title')]).toEqual([null, null]);
});
