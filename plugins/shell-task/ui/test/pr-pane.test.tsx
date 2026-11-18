// The PR pane on the real client and the mock peer, with wisp.ship's answer stubbed (the mock peer has no ship).
import { expect, test } from 'vitest';
import type { CommandDef } from '@wisp/client';
import { PrPane } from '../src/side/PrPane.tsx';
import { useTask } from '../src/task-context.tsx';
import { useShell } from '../src/shell.ts';
import { mountTask, part, until } from './helpers.tsx';
import type { PrPanel } from '../src/side/pr.ts';

const PR: PrPanel = {
  repo: 'o/r',
  number: 42,
  url: 'https://github.com/o/r/pull/42',
  title: 'Fix add in math.js',
  state: 'OPEN',
  branch: 'wisp/fix-add',
  base: 'main',
  author: 'wisp-bot',
  checks: { fail: 1, pending: 0, pass: 2 },
  checkList: [
    { name: 'lint', state: 'pass', ms: 12_000 },
    { name: 'test (node 22)', state: 'fail', ms: 72_000, url: 'https://ci/1' },
    { name: 'build', state: 'pass', ms: 30_000 },
  ],
  reviewers: [{ author: 'lena', state: 'APPROVED' }],
  notes: [
    { author: 'lena', body: 'Looks right; add a test for negatives?', at: 1 },
    { author: 'ci-bot', bot: true, body: 'Coverage 91%' },
  ],
  threads: [{ path: 'math.js', line: 2, notes: [{ author: 'lena', body: 'nit: spacing' }] }],
  additions: 3,
  deletions: 1,
  changedFiles: 1,
  files: [{ path: 'math.js', additions: 3, deletions: 1 }],
};

test('the_pr_pane_shows_the_verdict_failing_checks_first_the_passed_folded_and_the_conversation', async () => {
  const asked: string[] = [];
  const m = await mountTask({ sessionId: 's1', rows: {} }, () => {
    const task = useTask();
    // the task's row carries its PR (wisp.ship's `pr`): only then does the pane ask
    (task as { task: unknown }).task = () => ({ id: 's1', pr: { number: 42 } });
    // the pane's reads and writes go to wisp.ship: answer them here
    (task.client as { run: unknown }).run = (cmd: CommandDef<unknown, unknown>, args: unknown) => {
      asked.push(`${cmd.id} ${JSON.stringify(args)}`);
      return Promise.resolve(cmd.id === 'ship.details' ? [PR] : null);
    };
    return <PrPane />;
  });
  await until(() => part('side.pr.meta') !== null && part('side.pr.verdict.fail') !== null);
  expect(asked[0]).toBe(`ship.details {"key":"${m.key}","session":"s1"}`);
  expect(part('side.pr.verdict.fail')?.textContent).toContain('1 check failing');
  // the failing check is listed; the two that passed fold behind one line
  const rows = [...document.querySelectorAll('[data-kit="check-row"]')].map((r) => r.getAttribute('data-state'));
  expect(rows).toEqual(['fail']);
  const fold = part('side.pr.fold');
  expect(fold?.textContent).toBe('2 passed');
  fold?.click();
  await until(() => document.querySelectorAll('[data-kit="check-row"]').length === 3);
  expect([...document.querySelectorAll('[data-kit="check-row"]')].length).toBe(3);
  // reviewers, the conversation (the bot folded), the files, the foot
  expect(part('side.pr.reviews')?.textContent).toContain('Approved');
  expect(part('side.pr.talk')?.textContent).toContain('math.js:2');
  expect(part('side.pr.talk')?.textContent).not.toContain('Coverage 91%');
  expect(part('side.pr.talk')?.textContent).toContain('1 from bots');
  expect(part('side.pr.files')?.textContent).toContain('+3 −1');
  expect(part('side.pr.meta')?.textContent).toBe('#42 · Open · wisp/fix-add → main · by wisp-bot');
});

// audit A tasks #5: Merge asks first (v1 ship.js mergePr → ask()); a worktree the merge kept is said
test('merge_asks_first_and_says_when_the_worktree_was_kept', async () => {
  const asked: string[] = [];
  const shown: string[] = [];
  const ready: PrPanel = { ...PR, checks: { fail: 0, pending: 0, pass: 3 }, checkList: [{ name: 'lint', state: 'pass' }] };
  await mountTask({ sessionId: 's1', rows: {} }, () => {
    const task = useTask();
    const shell = useShell();
    (shell.toast as { show: unknown }).show = (m: string) => (shown.push(m), 'x');
    (task as { task: unknown }).task = () => ({ id: 's1', pr: { number: 42 } });
    (task.client as { run: unknown }).run = (cmd: CommandDef<unknown, unknown>) => {
      asked.push(cmd.id);
      if (cmd.id === 'ship.details') return Promise.resolve([ready]);
      return Promise.resolve(cmd.id === 'ship.merge' ? { kept: ['The worktree /w/t was kept: it has 1 uncommitted change.'] } : null);
    };
    return <PrPane />;
  });
  await until(() => part('side.pr.verdict.pass') !== null);
  (part('side.pr.verdict.pass')?.querySelector('button') as HTMLButtonElement).click();
  await until(() => part('side.pr.mergeAsk.ok') !== null);
  expect(asked).not.toContain('ship.merge');
  (part('side.pr.mergeAsk.ok') as HTMLButtonElement).click();
  await until(() => shown.length > 0);
  expect(asked).toContain('ship.merge');
  expect(shown).toEqual(['The worktree /w/t was kept: it has 1 uncommitted change.']);
});
