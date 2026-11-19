// Audit F tasks #1: after a click on the Shell tab the keys stayed on the tab, and what was typed ran the task list's
// shortcuts (`e` marked the task done, the rest went to the next task's reply box). v1 (panel.js:1014, :370) focuses the
// terminal when the Shell opens and when a shell (re)starts.
import { expect, test } from 'vitest';
import type { CommandDef } from '@wisp/client';
import { ShellPane } from '../src/side/ShellPane.tsx';
import { useTask } from '../src/task-context.tsx';
import { mountTask, until } from './helpers.tsx';

const inTerm = (): boolean => !!document.activeElement?.closest('[data-kit="term"]');

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
