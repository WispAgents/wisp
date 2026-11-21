import { beforeAll, describe, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { frame, mount, part, tick } from './helpers.tsx';
import { Dialog, Sheet } from '../src/controls/Dialog.tsx';
import { Popover } from '../src/controls/Popover.tsx';
import { Tooltip } from '../src/controls/Tooltip.tsx';
import { Button } from '../src/controls/Button.tsx';
import { Input } from '../src/controls/Input.tsx';
import { KitProvider } from '../src/core/i18n.tsx';
import { preloadControls } from '../src/controls/lazy-part.ts';

// the app fetches the overlays' machines once its first screen is up (ADR-0011); a press before that: lazy-controls.test.tsx
beforeAll(() => preloadControls());

const key = (el: Element, k: string): void => {
  el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
};

describe('Dialog', () => {
  test('opens from its trigger, takes focus, Escape closes and focus returns', async () => {
    const onOpenChange = vi.fn();
    mount(() => (
      <Dialog title="Delete recipe" trigger={{ label: 'Delete…' }} onOpenChange={onOpenChange} part="recipe.delete" footer={<Button variant="danger">Delete recipe</Button>}>
        This can't be undone.
      </Dialog>
    ));
    const trigger = part('recipe.delete.trigger');
    expect(trigger).not.toBeNull();
    trigger?.focus();
    trigger?.click();
    await tick(30);
    await frame();
    const content = part('recipe.delete');
    expect(content?.getAttribute('role')).toBe('dialog');
    expect(part('recipe.delete.title')?.textContent).toBe('Delete recipe');
    expect(content?.getAttribute('aria-labelledby')).toBe(part('recipe.delete.title')?.id);
    expect(content?.contains(document.activeElement)).toBe(true);
    expect(onOpenChange).toHaveBeenLastCalledWith(true);
    key(document.activeElement ?? document.body, 'Escape');
    // closed (Zag answers the key in a microtask) and focus back with it, no timer to wait on: dialog-focus-return.test.tsx
    await Promise.resolve();
    expect(part('recipe.delete')).toBeNull();
    expect(onOpenChange).toHaveBeenLastCalledWith(false);
    expect(document.activeElement).toBe(trigger);
  });

  // Audit E (as in D): Escape in Machines → Manage's Edit form closed the whole dialog. A field that answers Escape
  // itself (`onEscape`: an inline edit's Cancel) gets it, and the dialog stays; the next Escape, outside it, closes it
  test('escape_in_a_field_that_answers_it_stays_in_the_dialog', async () => {
    const onOpenChange = vi.fn();
    const onEscape = vi.fn();
    mount(() => (
      <Dialog title="Machines" open onOpenChange={onOpenChange} part="machines.manage">
        <Input aria-label="SSH address" part="edit.address" value="qa@192.0.2.1" onEscape={onEscape} />
        <Input aria-label="Other" part="other.field" value="" />
      </Dialog>
    ));
    await tick(30);
    await frame();
    const field = part('edit.address')?.querySelector('input') ?? part('edit.address');
    (field as HTMLElement | null)?.focus();
    key(field ?? document.body, 'Escape');
    await Promise.resolve();
    expect(onEscape).toHaveBeenCalledTimes(1);
    expect(onOpenChange).not.toHaveBeenCalledWith(false);
    expect(part('machines.manage')).not.toBeNull();
    const other = part('other.field')?.querySelector('input') ?? part('other.field');
    (other as HTMLElement | null)?.focus();
    key(other ?? document.body, 'Escape');
    await Promise.resolve();
    expect(onOpenChange).toHaveBeenLastCalledWith(false);
  });

  test('controlled: open prop shows it, the close button asks to close with a translated name', async () => {
    const [open, setOpen] = createSignal(true);
    mount(() => (
      <KitProvider t={(k) => (k === 'close' ? 'Fermer' : k)}>
        <Dialog title="Settings" open={open()} onOpenChange={setOpen} role="alertdialog" size="sm">
          body
        </Dialog>
      </KitProvider>
    ));
    await tick(30);
    expect(part('ui.dialog')?.getAttribute('role')).toBe('alertdialog');
    expect(part('ui.dialog')?.getAttribute('data-size')).toBe('sm');
    const close = part('ui.dialog.close');
    expect(close?.getAttribute('aria-label')).toBe('Fermer');
    close?.click();
    await tick(30);
    expect(open()).toBe(false);
    expect(part('ui.dialog')).toBeNull();
  });

  test('Sheet renders its own part and a grab handle', async () => {
    mount(() => <Sheet title="Pick a model" open={true}>list</Sheet>);
    await tick(30);
    expect(part('ui.sheet')?.getAttribute('data-kit')).toBe('sheet');
    expect(part('ui.sheet')?.querySelector('[data-kit="sheet-handle"]')).not.toBeNull();
  });
});

describe('Popover', () => {
  test('opens on click and closes on Escape, back to the trigger', async () => {
    mount(() => (
      <Popover trigger={{ icon: 'info', 'aria-label': 'About' }} title="About" part="about.pop">
        wisp
      </Popover>
    ));
    const trigger = part('about.pop.trigger');
    expect(trigger?.getAttribute('aria-label')).toBe('About');
    trigger?.click();
    await tick(30);
    await frame();
    expect(part('about.pop')?.getAttribute('data-state')).toBe('open');
    expect(part('about.pop.title')?.textContent).toBe('About');
    key(part('about.pop') ?? document.body, 'Escape');
    await tick(30);
    expect(part('about.pop')?.getAttribute('data-state')).toBe('closed');
  });
});

describe('Tooltip', () => {
  test('wraps its child and opens on keyboard focus', async () => {
    mount(() => (
      <Tooltip content="Archive" openDelay={0}>
        <Button>Go</Button>
      </Tooltip>
    ));
    expect(part('ui.tooltip.trigger')?.querySelector('button')).not.toBeNull();
    // Zag opens on focus only when it's keyboard focus: a Tab press sets that modality.
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
    part('ui.tooltip.trigger')?.querySelector('button')?.focus();
    await tick(30);
    expect(part('ui.tooltip')?.textContent).toBe('Archive');
  });
});
