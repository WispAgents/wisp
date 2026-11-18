import { describe, expect, test, vi } from 'vitest';
import { mount, part, tick } from './helpers.tsx';
import { Tabs } from '../src/controls/Tabs.tsx';
import { PlatformLookProvider } from '../src/platform/PlatformLook.tsx';
import { Split, Splitter } from '../src/controls/Splitter.tsx';

const key = (el: Element | null, k: string): void => {
  el?.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
};

const items = [
  { value: 'changes', label: 'Changes', content: <p>diff</p> },
  { value: 'pr', label: 'PR', count: 3, content: <p>pr</p> },
  { value: 'shell', label: 'Shell', content: <p>shell</p> },
];

describe('Tabs', () => {
  test('renders its parts; the first tab is selected and its panel shows', () => {
    mount(() => <Tabs items={items} aria-label="Side panel" part="side.tabs" />);
    expect(part('side.tabs.list')?.getAttribute('role')).toBe('tablist');
    const tabs = document.querySelectorAll('[data-part="side.tabs.tab"]');
    expect(tabs.length).toBe(3);
    expect(tabs[0]?.getAttribute('aria-selected')).toBe('true');
    const panels = [...document.querySelectorAll<HTMLElement>('[data-part="side.tabs.panel"]')];
    expect(panels.map((p) => p.hidden)).toEqual([false, true, true]);
  });

  test('ArrowRight, ArrowLeft, Home and End move the selection', async () => {
    const onChange = vi.fn();
    mount(() => <Tabs items={items} onChange={onChange} aria-label="Side panel" />);
    const tab = (v: string) => document.querySelector<HTMLElement>(`[data-part="ui.tabs.tab"][data-value="${v}"]`);
    const selected = () => document.querySelector('[data-part="ui.tabs.tab"][aria-selected="true"]')?.getAttribute('data-value');
    tab('changes')?.focus();
    await tick(10);
    key(document.activeElement, 'ArrowRight');
    await tick(10);
    expect(selected()).toBe('pr');
    key(document.activeElement, 'End');
    await tick(10);
    expect(selected()).toBe('shell');
    key(document.activeElement, 'Home');
    await tick(10);
    expect(selected()).toBe('changes');
    key(document.activeElement, 'ArrowLeft');
    await tick(10);
    expect(selected()).toBe('shell');
    expect(onChange).toHaveBeenLastCalledWith('shell');
  });
});

describe('Splitter and Split', () => {
  test('a splitter renders a panel per entry and a keyboard grip between each pair', () => {
    mount(() => (
      <Splitter
        panels={[
          { id: 'a', content: <p>a</p>, label: 'List' },
          { id: 'b', content: <p>b</p> },
          { id: 'c', content: <p>c</p> },
        ]}
      />
    ));
    expect(document.querySelectorAll('[data-part="ui.splitter.panel"]').length).toBe(3);
    const grips = document.querySelectorAll('[data-part="ui.splitter.grip"]');
    expect(grips.length).toBe(2);
    expect(grips[0]?.getAttribute('role')).toBe('separator');
    expect(grips[0]?.getAttribute('aria-label')).toBe('Resize: List');
    expect(grips[0]?.getAttribute('tabindex')).toBe('0');
  });

  test('Split side by side has a grip; stacked has none', () => {
    mount(() => <Split start={<p>list</p>} end={<p>task</p>} part="shell.split" />);
    expect(part('shell.split')?.getAttribute('data-kit')).toBe('splitter');
    expect(part('shell.split.grip')).not.toBeNull();
    document.body.innerHTML = '';
    mount(() => <Split stacked start={<p>list</p>} end={<p>task</p>} part="shell.split" />);
    expect(part('shell.split')?.getAttribute('data-kit')).toBe('split-stacked');
    expect(part('shell.split.start')?.textContent).toBe('list');
    expect(part('shell.split.grip')).toBeNull();
  });
});

// QA r7 #5: a phone's tab row that runs out of room scrolls, marked at the ends it can still scroll to (the look's
// stylesheet fades them, so no name is ever cut hard), and the open tab is scrolled into view. The browser's tabs are
// left as they were (no marks).
describe('a phone tab row that overflows (QA r7 #5)', () => {
  const sized = (el: HTMLElement, scrollWidth: number, clientWidth: number): void => {
    Object.defineProperty(el, 'scrollWidth', { configurable: true, get: () => scrollWidth });
    Object.defineProperty(el, 'clientWidth', { configurable: true, get: () => clientWidth });
  };
  for (const look of ['android', 'ios', 'web'] as const) {
    test(`on_${look}_the_rows_scroll_ends_are_${look === 'web' ? 'not_marked' : 'marked'}`, async () => {
      mount(() => (
        <PlatformLookProvider look={() => look}>
          <Tabs items={items} aria-label="Side panel" part="side.tabs" />
        </PlatformLookProvider>
      ));
      const root = part('side.tabs') as HTMLElement;
      sized(root, 300, 200);
      root.scrollLeft = 0;
      root.dispatchEvent(new Event('scroll'));
      await tick(5);
      expect(root.hasAttribute('data-scroll-end')).toBe(look !== 'web');
      expect(root.hasAttribute('data-scroll-start')).toBe(false);
      root.scrollLeft = 100;
      root.dispatchEvent(new Event('scroll'));
      await tick(5);
      expect(root.hasAttribute('data-scroll-start')).toBe(look !== 'web');
      expect(root.hasAttribute('data-scroll-end')).toBe(false);
    });
  }

  test('on_a_phone_the_open_tab_is_scrolled_into_view', async () => {
    mount(() => (
      <PlatformLookProvider look={() => 'android'}>
        <Tabs items={items} aria-label="Side panel" part="side.tabs" />
      </PlatformLookProvider>
    ));
    const root = part('side.tabs') as HTMLElement;
    sized(root, 300, 200);
    const shell = document.querySelector<HTMLElement>('[data-part="side.tabs.tab"][data-value="shell"]') as HTMLElement;
    root.getBoundingClientRect = () => ({ left: 0, right: 200, width: 200, top: 0, bottom: 48, height: 48, x: 0, y: 0, toJSON: () => ({}) }) as DOMRect;
    shell.getBoundingClientRect = () => ({ left: 240, right: 300, width: 60, top: 0, bottom: 48, height: 48, x: 240, y: 0, toJSON: () => ({}) }) as DOMRect;
    root.scrollLeft = 0;
    // the tab picked (a press, or the panel opened on it) is brought into the row’s view
    shell.click();
    await tick(20);
    expect(root.scrollLeft).toBe(100);
  });
});
