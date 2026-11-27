// A wisp key pressed before the shell's later part (the task and row keys, builtins-later.ts) is in runs once it lands,
// where it was pressed: it never starts deferred work early or loads a failed part again, never runs over an overlay
// that opened meanwhile, and an Enter on a button is the button's.
import { afterEach, expect, test } from 'vitest';
import type { ShellContext, ShortcutDef } from '../src/contract.ts';
import { installKeys } from '../src/keyboard.ts';
import type { Registry } from '../src/registry.ts';
import type { ShellWindow } from '../src/shell.tsx';

afterEach(() => {
  document.body.innerHTML = '';
});

function fake() {
  const overlays: unknown[] = [];
  let pending: Promise<void> | undefined;
  let land: () => void = () => {};
  const shortcuts: (ShortcutDef & { plugin: string })[] = [];
  const ran: string[] = [];
  let readyCalls = 0;
  const ctx = {
    nav: () => ({ main: { kind: 'task', id: 'laptop:a' }, side: { kind: 'none' }, overlays, layout: 'split' }),
    selection: { get: () => undefined },
    platform: { apple: true },
    laterReady: () => (readyCalls++, pending),
    laterPending: () => pending,
  } as unknown as ShellContext;
  const registry = { shortcuts: () => shortcuts } as unknown as Registry;
  const stop = installKeys(window as unknown as ShellWindow, ctx, registry, () => ({}) as never);
  return {
    stop,
    overlays,
    ran,
    readyCalls: () => readyCalls,
    loading: () => {
      pending = new Promise<void>((r) => (land = r));
    },
    lands: async () => {
      // the later part's keys register, then its load settles
      shortcuts.push({ id: 'shell.task.fork', keys: 'shift+f', label: 'x', group: 'task', plugin: 'wisp.shell', run: () => void ran.push('fork') } as never);
      land();
      pending = undefined;
      await new Promise((r) => setTimeout(r, 0));
    },
  };
}

const shiftF = (target: EventTarget = document.body): void =>
  void target.dispatchEvent(new KeyboardEvent('keydown', { key: 'F', code: 'KeyF', shiftKey: true, bubbles: true, cancelable: true }));

test('a_key_pressed_before_the_later_part_runs_once_it_lands_and_starts_nothing_early', async () => {
  const f = fake();
  f.loading();
  shiftF();
  expect(f.ran).toEqual([]);
  await f.lands();
  expect(f.ran).toEqual(['fork']);
  // laterReady (start every deferred thing now, try failed parts again) was never asked for
  expect(f.readyCalls()).toBe(0);
  f.stop();
});

test('with_nothing_on_its_way_an_unhandled_key_is_dropped', async () => {
  const f = fake();
  shiftF();
  await f.lands();
  expect(f.ran).toEqual([]);
  f.stop();
});

test('an_overlay_opened_meanwhile_keeps_the_early_key_from_running', async () => {
  const f = fake();
  f.loading();
  shiftF();
  f.overlays.push({ kind: 'palette' });
  await f.lands();
  expect(f.ran).toEqual([]);
  f.stop();
});

test('an_enter_on_a_button_is_the_buttons_and_never_kept', async () => {
  const f = fake();
  f.loading();
  const b = document.createElement('button');
  document.body.append(b);
  b.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
  await f.lands();
  expect(f.ran).toEqual([]);
  f.stop();
});
