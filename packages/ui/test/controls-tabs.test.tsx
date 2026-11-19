import { describe, expect, test, vi } from 'vitest';
import { mount, part, tick } from './helpers.tsx';
import { createSignal } from 'solid-js';
import { Tabs, fitPlan } from '../src/controls/Tabs.tsx';
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
  // the side panel (audit 2026-11-19): its items are made afresh on every change (a count, the open tab), and the kit
  // rebuilt every button, so → moved one tab and the focus fell to the page: the next → did nothing
  test('arrow_keys_keep_walking_when_the_items_are_made_afresh_on_each_change', async () => {
    const [value, setValue] = createSignal('changes');
    mount(() => <Tabs items={items.map((i) => ({ value: i.value, label: i.label, ...(i.value === value() && { title: 'open' }) }))} value={value()} onChange={setValue} aria-label="Side panel" />);
    const tab = (v: string) => document.querySelector<HTMLElement>(`[data-part="ui.tabs.tab"][data-value="${v}"]`);
    tab('changes')?.focus();
    await tick(10);
    const before = tab('pr');
    key(document.activeElement, 'ArrowRight');
    await tick(10);
    expect(value()).toBe('pr');
    expect(tab('pr'), 'the same button: not rebuilt').toBe(before);
    expect(document.activeElement?.getAttribute('data-value')).toBe('pr');
    key(document.activeElement, 'ArrowRight');
    await tick(10);
    expect(value()).toBe('shell');
  });

  // owner 2026-11-19, follow-up: at 1280 (the side panel 396 px, 297 px for its tabs plus the row's -8px) Changes, PR,
  // Shell and Preview stay labelled: Thread, Watch and pinned tabs go to their icon first, then the row tightens, then
  // Preview goes, then the rest, one at a time and only as far as needed. Widths as Chrome measured them in the dev app.
  const panel = {
    // Changes, PR, Shell (open), Preview, Thread (minor)
    normal: { widths: [88.6, 52.4, 65.4, 83.2, 78.3], icon: 32, gap: 2 },
    tight: { widths: [79.6, 43.4, 56.4, 74.2, 69.3], icon: 28, gap: 1 },
    chrome: 6,
    open: 2,
    minor: [false, false, false, false, true],
  };
  test('fit_at_396px_keeps_changes_pr_shell_and_preview_labelled_thread_gives_way_first', () => {
    // the labelled row needs 6 + 8 + 367.9 = 381.9: the dev app at 1280 has 297 + 8
    expect(fitPlan({ ...panel, room: 305 })).toEqual({ iconic: 1, tight: true });
    // 1440 (the panel 480 px): everything labelled, at v1's own padding
    expect(fitPlan({ ...panel, room: 389 })).toEqual({ iconic: 0, tight: false });
    // a little less: Thread to its icon first, still at v1's padding
    expect(fitPlan({ ...panel, room: 340 })).toEqual({ iconic: 1, tight: false });
  });

  test('fit_gives_way_from_the_end_one_tab_at_a_time_preview_before_the_rest_the_open_tab_last_of_all', () => {
    // Thread, then Preview, then PR, then Changes; Shell (open) keeps its label
    expect(fitPlan({ ...panel, room: 260 })).toEqual({ iconic: 2, tight: true });
    expect(fitPlan({ ...panel, room: 230 })).toEqual({ iconic: 3, tight: true });
    expect(fitPlan({ ...panel, room: 200 })).toEqual({ iconic: 4, tight: true });
    expect(fitPlan({ ...panel, room: 50 }), 'nothing more to give').toEqual({ iconic: 4, tight: true });
    // no minor tab: the row tightens before any tab goes
    expect(fitPlan({ ...panel, minor: [false, false, false, false, false], room: 305 })).toEqual({ iconic: 1, tight: true });
    expect(fitPlan({ ...panel, minor: [false, false, false, false, false], room: 360 })).toEqual({ iconic: 0, tight: true });
  });

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
