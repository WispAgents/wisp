// Audit F tasks #2: Revert in Changes threw the work away at once, with no question and no Undo (an untracked new file
// was lost for good). v1 (features/changes.js:519-582): a hunk goes at once and toasts "Reverted · Undo"; a whole file
// asks first, in the danger style, saying what goes; Undo puts back the text the file had just before, if it's still
// as the revert left it. Tasks #3: after Commit, the committed files stayed hidden ("No changes yet") until a reload.
import { expect, test } from 'vitest';
import type { CommandDef } from '@wisp/client';
import type { ToastOptions } from '@wisp/ui';
import { ChangesPane } from '../src/side/ChangesPane.tsx';
import { baseName, revertAsk, undoSteps } from '../src/side/revert.ts';
import { useShell } from '../src/shell.ts';
import { useTask } from '../src/task-context.tsx';
import en from '../src/i18n/en.lazy.json' with { type: 'json' };
import { mountTask, part, parts, until } from './helpers.tsx';

const t = (k: string, v?: Record<string, string | number>): string =>
  Object.entries(v ?? {}).reduce((s, [n, x]) => s.replaceAll(`{${n}}`, String(x)), (en as Record<string, string>)[k] ?? k);

test('the_revert_question_says_what_goes_and_undo_puts_back_the_text_from_before_audit_f_tasks_2', () => {
  expect(revertAsk('src/math.js', 'M', 'origin/main', t)).toEqual({
    title: 'Revert src/math.js to origin/main?',
    body: 'Every change this task made to math.js goes. Undo puts it back.',
  });
  expect(revertAsk('new.txt', 'A', 'origin/main', t).body).toBe('new.txt is new in this task, so reverting deletes it. Undo brings it back.');
  expect(revertAsk('old.txt', 'D', '', t)).toEqual({ title: 'Revert old.txt to the base?', body: 'old.txt comes back as it was before the task. Undo puts it back.' });
  // the text read before; a rename's old name (which the revert brought back) goes again; unreadable: no Undo
  expect(undoSteps('b.js', 'a.js', 'x', 'h1')).toEqual([
    { path: 'b.js', text: 'x', hash: 'h1' },
    { path: 'a.js', text: null },
  ]);
  expect(undoSteps('gone.js', undefined, null, 'h2')).toEqual([{ path: 'gone.js', text: null, hash: 'h2' }]);
  expect(undoSteps('img.png', undefined, undefined, 'h3')).toBeNull();
  // the base as the user knows it: origin/main; with no remote, the branch it came from, never a 40-letter sha
  const sha = '5b0383d0679ce4e5f52aa9d2f6eef2754aadae5b';
  expect(baseName('origin/main', 'origin/main', 'main')).toBe('origin/main');
  expect(baseName(sha, sha, 'main')).toBe('main');
  expect(baseName(sha, undefined, undefined)).toBe('5b0383d');
  expect(baseName(undefined, 'HEAD', undefined)).toBe('');
});

const ROWS = [
  { op: ' ', text: 'a', a: 1, b: 1 },
  { op: '-', text: 'b', a: 2 },
  { op: '+', text: 'B', b: 2 },
];

async function mountChanges(files: { path: string; status: string }[]) {
  const calls: { id: string; args: Record<string, unknown> }[] = [];
  const toasts: { message: string; options?: ToastOptions }[] = [];
  let listed = files;
  await mountTask({ sessionId: 's1', rows: {}, plugin: 'shell-task' }, () => {
    const task = useTask();
    const shell = useShell();
    (task as { task: unknown }).task = () => ({ id: 's1', title: 'T', summary: { worktree: { path: '/w/s1', baseRef: 'origin/main' } } });
    (shell.toast as { show: unknown }).show = (message: string, options?: ToastOptions) => (toasts.push({ message, ...(options && { options }) }), 'id');
    (task.client as { run: unknown }).run = (cmd: CommandDef<unknown, unknown>, args: Record<string, unknown>) => {
      calls.push({ id: cmd.id, args });
      if (cmd.id === 'git.changes')
        return Promise.resolve({
          base: 'origin/main',
          body: { kind: 'inline', files: listed.map((f) => ({ ...f, add: 1, del: 1, binary: false, rows: ROWS, staged: false, unstaged: true })) },
        });
      if (cmd.id === 'worktrees.source') return Promise.resolve({ text: 'a\nB\n', hash: 'before' });
      if (cmd.id === 'git.revert') return Promise.resolve({ ok: true, hash: 'after' });
      if (cmd.id === 'git.commit') return Promise.resolve({ ok: true, sha: 'abc1234' });
      return Promise.resolve(null);
    };
    return <ChangesPane />;
  });
  await until(() => parts('changes.file').length === files.length);
  return { calls, toasts, relist: (f: typeof files) => (listed = f) };
}

test('a_hunks_revert_toasts_reverted_with_undo_which_puts_the_text_back_audit_f_tasks_2', async () => {
  const m = await mountChanges([{ path: 'math.js', status: 'M' }]);
  part('panel.changes.revertHunk')?.click();
  await until(() => m.toasts.length > 0);
  expect(m.calls.find((c) => c.id === 'git.revert')?.args).toMatchObject({ path: 'math.js', hunk: 0 });
  const done = m.toasts[0];
  expect(done?.message).toBe('Reverted');
  expect(done?.options?.action?.label).toBe('Undo');
  done?.options?.action?.onClick();
  await until(() => m.calls.filter((c) => c.id === 'git.revert').length === 2);
  expect(m.calls.filter((c) => c.id === 'git.revert')[1]?.args).toMatchObject({ path: 'math.js', restore: 'a\nB\n', hash: 'after' });
});

test('revert_file_asks_first_and_cancel_keeps_the_file_audit_f_tasks_2', async () => {
  const m = await mountChanges([{ path: 'new.txt', status: 'A' }]);
  // the file's ⋯ → Revert file (the menu's choice, as ChangesList hands it over)
  const menu = part('changes.file')?.querySelector<HTMLElement>('[data-kit="menu-trigger"], [aria-haspopup="menu"]');
  menu?.click();
  await until(() => [...document.querySelectorAll('[role="menuitem"]')].some((e) => e.textContent?.includes('Revert file')));
  [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((e) => e.textContent?.includes('Revert file'))?.click();
  await until(() => part('side.changes.revertAsk') !== null);
  expect(document.body.textContent).toContain('Revert new.txt to origin/main?');
  expect(document.body.textContent).toContain('new.txt is new in this task, so reverting deletes it. Undo brings it back.');
  expect(m.calls.some((c) => c.id === 'git.revert')).toBe(false);
  part('side.changes.revertAsk.ok')?.click();
  await until(() => m.toasts.length > 0);
  expect(m.calls.find((c) => c.id === 'git.revert')?.args).toMatchObject({ path: 'new.txt' });
  expect(m.toasts[0]?.options?.action?.label).toBe('Undo');
});

test('committed_files_stay_listed_once_gits_read_is_in_audit_f_tasks_3', async () => {
  const m = await mountChanges([
    { path: 'a.js', status: 'M' },
    { path: 'b.js', status: 'A' },
  ]);
  const box = part('panel.changes.commit')?.querySelector<HTMLInputElement | HTMLTextAreaElement>('input, textarea');
  if (box) {
    box.value = 'msg';
    box.dispatchEvent(new Event('input', { bubbles: true }));
  }
  part('panel.changes.commit')?.querySelector<HTMLButtonElement>('button[type="submit"], button')?.click();
  await until(() => m.toasts.some((x) => x.message.startsWith('Committed')));
  // git still lists them vs origin/main: so does the pane
  await until(() => parts('changes.file').length === 2);
  expect(parts('changes.file').length).toBe(2);
});
