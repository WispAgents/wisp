// v1's keys (kbd.js) as the shell registers them: J/K walk the list's rows, E / A / Delete act on the picked row,
// and the open task's keys go to its page, which says what it handled.
import { afterEach, expect, test } from 'vitest';
import { shellLater } from '../src/builtins-later.ts';
import type { ShellContext, ShortcutDef } from '../src/contract.ts';
import { PENDING_KEY, PENDING_KEY_MS, TASK_KEY, takePendingKey } from '../src/frame/task-keys.ts';
import { isTyping } from '../src/keyboard.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

function fake(main: { kind: string; id?: string }) {
  let picked: string | undefined;
  const went: unknown[] = [];
  const ran: { cmd: string; args: unknown }[] = [];
  const toasts: string[] = [];
  const ctx = {
    nav: () => ({ main, side: { kind: 'none' }, overlays: [], layout: 'split' }),
    navigate: (i: unknown) => void went.push(i),
    selection: { get: () => picked, set: (k: string | undefined) => void (picked = k) },
    client: { commands: { run: (cmd: { id: string }, args: unknown) => (ran.push({ cmd: cmd.id, args }), Promise.resolve()) }, entities: { peek: () => undefined } },
    toast: { success: (m: string) => void toasts.push(m) },
    palette: { open: () => {} },
    sidebar: { toggle: () => {} },
    focusTargets: { focus: () => false },
    t: (k: string) => k,
  } as unknown as ShellContext;
  const keys = (shellLater(ctx).shortcuts ?? []) as ShortcutDef[];
  const press = (id: string): boolean | void => keys.find((s) => s.id === id)?.run({ event: new KeyboardEvent('keydown') } as never);
  return { ctx, keys, press, went, ran, toasts, picked: () => picked };
}

test('v1s_keys_walk_the_list_act_on_the_picked_row_and_reach_the_open_tasks_page', () => {
  document.body.innerHTML = ['a', 'b', 'c'].map((id) => `<article data-part="card" data-id="laptop:${id}"></article>`).join('');
  const f = fake({ kind: 'home' });
  // ~30 keys, each once (Enter and Backspace are the second key of Open and Delete)
  expect(new Set(f.keys.map((k) => k.keys)).size).toBeGreaterThanOrEqual(30);
  f.press('shell.next');
  expect(f.picked()).toBe('laptop:a');
  f.press('shell.next');
  f.press('shell.next');
  f.press('shell.next');
  expect(f.picked()).toBe('laptop:c'); // stops at the end
  f.press('shell.prev');
  expect(f.picked()).toBe('laptop:b');
  // E on the picked row: done, as its button does
  expect(f.press('shell.done')).toBe(true);
  expect(f.ran.at(-1)).toEqual({ cmd: 'runtime.setState', args: { id: 'laptop:b', state: 'done' } });
  expect(f.went.at(-1)).toEqual({ t: 'markDone', id: 'laptop:b' });
  // ... with Undo (audit A #13)
  expect(f.toasts.at(-1)).toBe('shell.toast.done');
  // O opens it
  f.press('shell.open');
  expect(f.went.at(-1)).toEqual({ t: 'openTask', id: 'laptop:b' });
  // a task key on the picked row with no task open: M opens Rename for it (audit A #12)
  const h = fake({ kind: 'home' });
  h.ctx.selection.set('laptop:c');
  expect(h.press('shell.task.rename')).toBe(true);
  expect(h.went.at(-1)).toEqual({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.rename', params: { id: 'laptop:c' } } });
  // Delete asks first (audit A #4); with no row picked a task key goes on
  expect(h.press('shell.delete')).toBe(true);
  expect(h.went.at(-1)).toEqual({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.delete', params: { id: 'laptop:c' } } });
  expect(h.ran).toEqual([]);
  h.ctx.selection.set(undefined);
  expect(h.press('shell.task.transcript')).toBe(false);

  // with a task open, its keys reach the page; what the page doesn't handle goes on
  const g = fake({ kind: 'task', id: 'laptop:a' });
  const heard: string[] = [];
  const page = (e: Event): void => {
    const act = (e as CustomEvent<string>).detail;
    heard.push(act);
    if (act !== 'answer:9') e.preventDefault();
  };
  document.addEventListener(TASK_KEY, page);
  expect(g.press('shell.task.transcript')).toBe(true);
  expect(g.press('shell.task.ship')).toBe(true);
  expect(g.press('shell.task.answer9')).toBe(false);
  // E with a task open: the page marks it done (its button), not the list
  expect(g.press('shell.done')).toBe(true);
  expect(heard).toEqual(['transcript', 'ship', 'answer:9', 'done']);
  expect(g.ran).toEqual([]);
  document.removeEventListener(TASK_KEY, page);
});

// review: a key left for a task's page that never opened doesn't fire on a later visit; renaming in place is typing
test('a_pending_key_is_good_once_and_briefly_and_a_title_being_renamed_is_typing', () => {
  Object.assign(PENDING_KEY, { id: 'laptop:a', act: 'changes', at: 1000 });
  expect(takePendingKey('laptop:b', 1001), 'another task’s page clears it').toBeUndefined();
  expect(takePendingKey('laptop:a', 1002)).toBeUndefined();
  Object.assign(PENDING_KEY, { id: 'laptop:a', act: 'changes', at: 1000 });
  expect(takePendingKey('laptop:a', 1000 + PENDING_KEY_MS + 1), 'too late').toBeUndefined();
  Object.assign(PENDING_KEY, { id: 'laptop:a', act: 'changes', at: 1000 });
  expect(takePendingKey('laptop:a', 1500)).toBe('changes');
  const h = document.createElement('h2');
  h.setAttribute('contenteditable', 'plaintext-only');
  document.body.append(h);
  expect(isTyping(h)).toBe(true);
});
