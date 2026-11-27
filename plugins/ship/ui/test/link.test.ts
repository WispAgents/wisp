// ADR-0145: which links in a task's turns read as one of its PRs (the `shell.task:link` extension's `if`).
import { expect, test } from 'vitest';
import { prKeyOf, prOfLink } from '../src/model/link.ts';
import type { ShipSession } from '../src/types.ts';

const pr = (n: number, url: string) => ({ number: n, url, state: 'OPEN' as const });
const task: ShipSession = {
  id: 'p:t1',
  agent: 'claude',
  state: 'idle',
  project: 'api',
  pr: pr(12, 'https://github.com/Acme/API/pull/12'),
  prs: [{ repo: 'docs', pr: pr(9, 'https://github.com/acme/docs/pull/9') }],
};

test('a PR link with any tail, scheme or case is the same PR', () => {
  const k = 'acme/api#12';
  for (const h of ['https://github.com/acme/api/pull/12', 'http://www.github.com/ACME/api/pull/12/files', 'https://github.com/acme/api/pull/12#discussion_r1', 'https://github.com/acme/api/pull/12?w=1']) {
    expect(prKeyOf(h), h).toBe(k);
  }
  for (const h of ['https://github.com/acme/api/issues/12', 'https://github.com/acme/api/pull/0', 'https://github.com/acme/api/pull/12x', 'https://gitlab.com/acme/api/pull/12', 'https://notgithub.com/acme/api/pull/12']) {
    expect(prKeyOf(h), h).toBeUndefined();
  }
});

test("only the task's own PRs (shipped or attached) become its chip", () => {
  expect(prOfLink(task, 'https://github.com/acme/api/pull/12/files')?.pr.number).toBe(12);
  expect(prOfLink(task, 'https://github.com/acme/docs/pull/9')?.repo).toBe('docs');
  expect(prOfLink(task, 'https://github.com/acme/docs/pull/10'), 'not on the task: stays a link').toBeUndefined();
  expect(prOfLink(undefined, 'https://github.com/acme/api/pull/12')).toBeUndefined();
  expect(prOfLink(task, undefined)).toBeUndefined();
});
