import { describe, expect, test, vi } from 'vitest';
import { mount, part, tick } from './helpers.tsx';
import { createToaster, type RollbackToaster } from '../src/controls/toaster.ts';
import { Toasts } from '../src/controls/Toast.tsx';

describe('toaster', () => {
  test('the same text twice shows once', async () => {
    const toaster = createToaster();
    const a = toaster.show('Archived');
    const b = toaster.show('Archived');
    expect(a).toBe(b);
    expect((await toaster.ready()).getVisibleToasts().length).toBe(1);
  });

  test('at most three on screen; the rest wait', async () => {
    const toaster = createToaster();
    for (const msg of ['Archived', 'Marked done', 'Saved for later', 'Reopened']) toaster.show(msg);
    expect((await toaster.ready()).getVisibleToasts().length).toBe(3);
  });

  test('error is the red one, and a command runner can call it through RollbackToaster', async () => {
    const toaster = createToaster();
    const sink: RollbackToaster = toaster;
    sink.error('Still working — stop it first or let it finish.', { action: { label: 'Stop', onClick: () => {} } });
    const [shown] = (await toaster.ready()).getVisibleToasts();
    expect(shown?.type).toBe('error');
    expect(shown?.action?.label).toBe('Stop');
  });
});

describe('<Toasts>', () => {
  test('renders each toast with its parts; the action runs and × has a name', async () => {
    const toaster = createToaster();
    const undo = vi.fn();
    mount(() => <Toasts toaster={toaster} />);
    toaster.show('Archived', { action: { label: 'Undo', onClick: undo } });
    await tick(30);
    expect(part('ui.toast.group')).not.toBeNull();
    expect(part('ui.toast')?.textContent).toContain('Archived');
    expect(part('ui.toast.close')?.getAttribute('aria-label')).toBe('Dismiss');
    part('ui.toast.action')?.click();
    await tick(10);
    expect(undo).toHaveBeenCalled();
  });

  test('dismiss plays the exit and then removes it; clear removes the rest', async () => {
    const toaster = createToaster();
    mount(() => <Toasts toaster={toaster} />);
    const id = toaster.show('Archived');
    toaster.show('Marked done');
    await tick(30);
    toaster.dismiss(id);
    await tick(400);
    const store = await toaster.ready();
    expect(store.isDismissed(id)).toBe(true);
    expect(store.getVisibleToasts().length).toBe(1);
    toaster.clear();
    await tick(400);
    expect(store.getVisibleToasts().length).toBe(0);
  });

  test('a toast shown before the store has loaded waits for it, and so does its dismiss (ADR-0011: it loads apart)', async () => {
    const toaster = createToaster();
    mount(() => <Toasts toaster={toaster} />);
    expect(toaster.store()).toBeUndefined();
    const kept = toaster.show('Archived');
    const gone = toaster.show('Marked done');
    toaster.dismiss(gone);
    const store = await toaster.ready();
    expect(toaster.store()).toBe(store);
    await tick(400);
    expect(store.isDismissed(gone)).toBe(true);
    expect(store.getVisibleToasts().map((x) => x.id)).toEqual([kept]);
    expect(part('ui.toast')?.textContent).toContain('Archived');
  });

  test('an error toast is marked as one', async () => {
    const toaster = createToaster();
    mount(() => <Toasts toaster={toaster} part="app.toast" />);
    toaster.error('Can’t reach wisp. Check the connection and try again.');
    await tick(30);
    expect(part('app.toast')?.getAttribute('data-type')).toBe('error');
  });
});
