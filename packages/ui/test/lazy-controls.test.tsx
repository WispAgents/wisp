// The overlays' machines load apart from the first page (ADR-0011, controls/lazy-part.ts). Here nothing preloads them:
// each control draws at once with plain props, a press before its machine shows it pressed (ADR-0017) and does what it
// would have once the machine lands, and the machine then takes over the same elements. Each test is the first to use
// its control's machine (a part loads once per page), so its "before" is real.
import { describe, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { mount, part, tick } from './helpers.tsx';
import { Menu } from '../src/controls/Menu.tsx';
import { Select } from '../src/controls/Select.tsx';
import { Dialog } from '../src/controls/Dialog.tsx';
import { Checkbox } from '../src/controls/Switch.tsx';

const key = (el: Element | null, k: string): void => {
  el?.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
};
/** a machine's chunk has loaded (the press asked for it: this waits on that same import), and what it replays after
 * binding has run */
const landed = async (machine: () => Promise<unknown>): Promise<void> => {
  await machine();
  await tick(10);
};

describe('a press before the machine', () => {
  test('Menu: the trigger draws at once with its roles; a click shows it pressed and opens the menu as the machine lands', async () => {
    const onSelect = vi.fn();
    mount(() => <Menu items={[{ value: 'rerun', label: 'Rerun…' }, { value: 'done', label: 'Mark done' }]} onSelect={onSelect} trigger={{ icon: 'dots', 'aria-label': 'More' }} part="task.more" />);
    const trigger = part('task.more.trigger');
    expect(trigger?.getAttribute('aria-haspopup')).toBe('menu');
    expect(trigger?.getAttribute('aria-expanded')).toBe('false');
    expect(part('task.more')).toBeNull();
    trigger?.click();
    // the same frame: pressed
    expect(trigger?.getAttribute('aria-expanded')).toBe('true');
    // check:allow dynamic-import-in-test waits on the lazy machine chunk the press asked for; a top import would preload it and lose the before-the-machine state
    await landed(() => import('../src/controls/menu-machine.tsx'));
    expect(part('task.more')?.getAttribute('data-state')).toBe('open');
    expect(document.querySelectorAll('[data-part="task.more.item"]').length).toBe(2);
    // the machine owns the trigger now: its id points at the menu
    expect(trigger?.getAttribute('aria-controls')).toBe(part('task.more')?.id);
    (document.querySelector('[data-value="done"]') as HTMLElement | null)?.click();
    await tick(20);
    expect(onSelect).toHaveBeenCalledWith('done');
  });

  test('Select: the picked label draws at once; ArrowDown before the machine opens the list with the keyboard', async () => {
    const [value, setValue] = createSignal<string | undefined>('codex');
    mount(() => <Select items={[{ value: 'claude', label: 'Claude Code' }, { value: 'codex', label: 'Codex' }]} value={value()} onChange={setValue} aria-label="Harness" />);
    const trigger = part('ui.select.trigger');
    expect(trigger?.textContent).toContain('Codex');
    expect(trigger?.getAttribute('aria-haspopup')).toBe('listbox');
    trigger?.focus();
    key(trigger, 'ArrowDown');
    expect(trigger?.getAttribute('aria-expanded')).toBe('true');
    // check:allow dynamic-import-in-test waits on the lazy machine chunk the press asked for; a top import would preload it and lose the before-the-machine state
    await landed(() => import('../src/controls/select-machine.tsx'));
    expect(part('ui.select.list')?.getAttribute('data-state')).toBe('open');
    expect(trigger?.textContent).toContain('Codex');
  });

  test('Dialog: open draws its surface at once (its role, title, body), focus goes in; Escape and × close it before the machine is there', async () => {
    const onOpenChange = vi.fn();
    mount(() => (
      <Dialog title="Rename task" open onOpenChange={onOpenChange}>
        <p>body</p>
      </Dialog>
    ));
    const box = part('ui.dialog');
    expect(box?.getAttribute('role')).toBe('dialog');
    expect(part('ui.dialog.title')?.textContent).toBe('Rename task');
    expect(part('ui.dialog.body')?.textContent).toBe('body');
    await Promise.resolve();
    expect(document.activeElement).toBe(box);
    key(box, 'Escape');
    expect(onOpenChange).toHaveBeenLastCalledWith(false);
    onOpenChange.mockClear();
    part('ui.dialog.close')?.click();
    expect(onOpenChange).toHaveBeenLastCalledWith(false);
  });

  test('Checkbox: a plain input that already works; the machine binds the same box when it lands', async () => {
    const onChange = vi.fn();
    mount(() => <Checkbox checked={false} onChange={onChange} label="Keep worktree" />);
    const input = document.querySelector<HTMLInputElement>('[data-kit="checkbox"] input');
    expect(part('ui.checkbox.control')?.getAttribute('data-state')).toBe('unchecked');
    input?.click();
    expect(onChange).toHaveBeenLastCalledWith(true);
    // a hover fetches its machine (the app's preload would have)
    part('ui.checkbox')?.dispatchEvent(new PointerEvent('pointerenter'));
    // check:allow dynamic-import-in-test waits on the lazy machine chunk the press asked for; a top import would preload it and lose the before-the-machine state
    await landed(() => import('../src/controls/checkbox-machine.ts'));
    expect(part('ui.checkbox')?.getAttribute('data-scope')).toBe('checkbox');
  });
});
