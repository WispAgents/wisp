// Audit F tasks #1: after a click on the Shell tab the keys stayed on the tab, and what was typed ran the task list's
// shortcuts (`e` marked the task done, the rest went to the next task's reply box). v1 (panel.js:1014, :370) focuses the
// terminal when the Shell opens and when a shell (re)starts.
import { expect, test } from 'vitest';
import type { CommandDef } from '@wisp/client';
import { ShellPane, shellWalkedTo, walksTabs } from '../src/side/ShellPane.tsx';
import { useTask } from '../src/task-context.tsx';
import { useShell } from '../src/shell.ts';
import { mountTask, until } from './helpers.tsx';

const inTerm = (): boolean => !!document.activeElement?.closest('[data-kit="term"]');

/** mounts a Shell whose start answers at once; `ran` the commands it ran */
async function mountShell(ran: string[]): Promise<void> {
  await mountTask({ sessionId: 's1', rows: {} }, () => {
    const task = useTask();
    (task.client as { run: unknown }).run = (cmd: CommandDef<unknown, unknown>) => {
      ran.push(cmd.id);
      if (cmd.id === 'terminal.shell.info') return Promise.resolve({ supported: true, open: false, app: true, name: 'zsh' });
      if (cmd.id === 'terminal.shell.start') return Promise.resolve({ ok: true, started: true });
      return Promise.resolve({ ok: true });
    };
    (task.client as { http: unknown }).http = () => ({ events: async function* () {} });
    return <ShellPane kind="shell" />;
  });
}

test('an_arrow_key_onto_shell_starts_it_but_leaves_the_keys_on_the_tab_audit_g_tasks_3', async () => {
  // → along the strip selected Shell: its tab has the focus, and must keep it so the next → walks on
  const strip = document.createElement('div');
  strip.dataset.part = 'panel.tabs';
  const tab = document.createElement('button');
  strip.append(tab);
  document.body.append(strip);
  tab.focus();
  expect(walksTabs('ArrowRight') && walksTabs('Home') && !walksTabs('Enter') && !walksTabs(' ')).toBe(true);
  shellWalkedTo();
  const ran: string[] = [];
  await mountShell(ran);
  await until(() => ran.includes('terminal.shell.start'));
  // a few frames for a focus that would follow the start
  await new Promise((r) => setTimeout(r, 120));
  expect(document.activeElement).toBe(tab);
  expect(inTerm()).toBe(false);
});

test('a_deep_link_to_the_shell_takes_the_keys_from_the_reply_box_audit_g_tasks_f1', async () => {
  // the page opened on ?side=shell:…: the reply box focused itself on load, the user hasn't pressed anything yet
  const reply = document.createElement('textarea');
  document.body.append(reply);
  reply.focus();
  const nav = navigator as { userActivation?: unknown };
  const had = Object.getOwnPropertyDescriptor(navigator, 'userActivation');
  Object.defineProperty(navigator, 'userActivation', { configurable: true, value: { hasBeenActive: false, isActive: false } });
  try {
    const ran: string[] = [];
    await mountShell(ran);
    await until(() => ran.includes('terminal.shell.start') && inTerm());
    expect(inTerm()).toBe(true);
  } finally {
    if (had) Object.defineProperty(navigator, 'userActivation', had);
    else delete nav.userActivation;
  }
});

test('a_reply_box_the_user_moved_to_keeps_the_keys_when_the_shell_starts', async () => {
  const reply = document.createElement('textarea');
  document.body.append(reply);
  reply.focus();
  Object.defineProperty(navigator, 'userActivation', { configurable: true, value: { hasBeenActive: true, isActive: false } });
  try {
    const ran: string[] = [];
    await mountShell(ran);
    await until(() => ran.includes('terminal.shell.start'));
    await new Promise((r) => setTimeout(r, 120));
    expect(document.activeElement).toBe(reply);
  } finally {
    delete (navigator as { userActivation?: unknown }).userActivation;
  }
});

test('opening_the_shell_gives_the_terminal_the_keys_audit_f_tasks_1', async () => {
  // the Shell tab was clicked: it has the focus when the pane opens
  const tab = document.createElement('button');
  document.body.append(tab);
  tab.focus();
  const ran: string[] = [];
  await mountTask({ sessionId: 's1', rows: {} }, () => {
    const task = useTask();
    (task.client as { run: unknown }).run = (cmd: CommandDef<unknown, unknown>) => {
      ran.push(cmd.id);
      if (cmd.id === 'terminal.shell.info') return Promise.resolve({ supported: true, open: false, app: true, name: 'zsh' });
      if (cmd.id === 'terminal.shell.start') return Promise.resolve({ ok: true, started: true });
      return Promise.resolve({ ok: true });
    };
    // the shell's output: none yet
    (task.client as { http: unknown }).http = () => ({ events: async function* () {} });
    return <ShellPane kind="shell" />;
  });
  await until(() => ran.includes('terminal.shell.start') && inTerm());
  expect(document.activeElement).not.toBe(tab);
  expect(inTerm()).toBe(true);
});

test('a_shell_that_draws_late_still_gets_the_keys_it_was_asked_for_audit_h_tasks_g3', async () => {
  // WebKit on a loaded machine: the terminal's input is under the "Starting…" screen (visibility: hidden, so focus is
  // refused) until the shell's first output, which came after the pane's half second of retries: Enter on Shell left
  // the keys on the tab. Here the input refuses focus until the first output, 900 ms in.
  const tab = document.createElement('button');
  document.body.append(tab);
  tab.focus();
  let hidden = true;
  const real = HTMLTextAreaElement.prototype.focus;
  HTMLTextAreaElement.prototype.focus = function (this: HTMLTextAreaElement, o?: FocusOptions) {
    if (hidden && this.dataset.kit === 'term-input') return;
    real.call(this, o);
  };
  try {
    const ran: string[] = [];
    await mountTask({ sessionId: 's1', rows: {} }, () => {
      const task = useTask();
      (task.client as { run: unknown }).run = (cmd: CommandDef<unknown, unknown>) => {
        ran.push(cmd.id);
        if (cmd.id === 'terminal.shell.info') return Promise.resolve({ supported: true, open: false, app: true, name: 'zsh' });
        if (cmd.id === 'terminal.shell.start') return Promise.resolve({ ok: true, started: true });
        return Promise.resolve({ ok: true });
      };
      (task.client as { http: unknown }).http = () => ({
        events: async function* () {
          await new Promise((r) => setTimeout(r, 900));
          hidden = false; // the first draw shows the rows, and the input with them
          yield { event: 'data', data: JSON.stringify('$ ') };
          await new Promise(() => undefined);
        },
      });
      return <ShellPane kind="shell" />;
    });
    await until(() => ran.includes('terminal.shell.start'));
    await new Promise((r) => setTimeout(r, 1300));
    expect(inTerm(), `focus on ${document.activeElement?.tagName}`).toBe(true);
  } finally {
    HTMLTextAreaElement.prototype.focus = real;
  }
});

test('a_kept_request_is_dropped_when_the_user_moves_the_keys_elsewhere_review_of_audit_h_tasks_g3', async () => {
  // a deep link to the Shell (a fresh page: the request is the page's own), the shell slow to draw: meanwhile the user
  // Tabs to the reply box and types. When the shell draws, the keys stay where the user put them.
  const reply = document.createElement('textarea');
  document.body.append(reply);
  Object.defineProperty(navigator, 'userActivation', { configurable: true, value: { hasBeenActive: false, isActive: false } });
  let hidden = true;
  const real = HTMLTextAreaElement.prototype.focus;
  HTMLTextAreaElement.prototype.focus = function (this: HTMLTextAreaElement, o?: FocusOptions) {
    if (hidden && this.dataset.kit === 'term-input') return;
    real.call(this, o);
  };
  try {
    const ran: string[] = [];
    await mountTask({ sessionId: 's1', rows: {} }, () => {
      const task = useTask();
      (task.client as { run: unknown }).run = (cmd: CommandDef<unknown, unknown>) => {
        ran.push(cmd.id);
        if (cmd.id === 'terminal.shell.info') return Promise.resolve({ supported: true, open: false, app: true, name: 'zsh' });
        if (cmd.id === 'terminal.shell.start') return Promise.resolve({ ok: true, started: true });
        return Promise.resolve({ ok: true });
      };
      (task.client as { http: unknown }).http = () => ({
        events: async function* () {
          await new Promise((r) => setTimeout(r, 900));
          hidden = false;
          yield { event: 'data', data: JSON.stringify('$ ') };
          await new Promise(() => undefined);
        },
      });
      return <ShellPane kind="shell" />;
    });
    await until(() => ran.includes('terminal.shell.start'));
    await new Promise((r) => setTimeout(r, 650)); // past the retries: the request is kept
    reply.focus(); // the user moved on, by the keys
    await new Promise((r) => setTimeout(r, 700)); // the shell draws
    expect(document.activeElement).toBe(reply);
  } finally {
    HTMLTextAreaElement.prototype.focus = real;
    delete (navigator as { userActivation?: unknown }).userActivation;
  }
});

test('a_shell_that_cant_start_hands_the_caret_to_the_reply_box_review_of_audit_h_tasks_f1', async () => {
  // the page opened on the Shell leaves the caret to it (TaskView): a computer with no shell gives it to the reply box
  const reply = document.createElement('textarea');
  document.body.append(reply);
  await mountTask({ sessionId: 's1', rows: {} }, () => {
    const task = useTask();
    const shell = useShell() as { prompts?: unknown; nav: () => { layout: string } };
    shell.prompts = { get: (n: string) => (n === 'reply' ? { focus: () => reply.focus() } : undefined), provide: () => () => undefined };
    const nav = shell.nav;
    shell.nav = () => ({ ...nav(), layout: 'split' });
    (task.client as { run: unknown }).run = (cmd: CommandDef<unknown, unknown>) =>
      cmd.id === 'terminal.shell.info' ? Promise.resolve({ supported: false, open: false, app: false, name: '' }) : Promise.resolve({ ok: true });
    return <ShellPane kind="shell" />;
  });
  await until(() => document.activeElement === reply);
  expect(document.activeElement).toBe(reply);
});
