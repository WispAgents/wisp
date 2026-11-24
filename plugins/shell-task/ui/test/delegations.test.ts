import { expect, test } from 'vitest';
import { delegationTime, delegationsOf } from '../src/cards/delegations.ts';
import type { TurnDoc } from '../src/cards/model.ts';

const STRINGS: Readonly<Record<string, string>> = {
  'shell-task.delegations.working': 'Working',
  'shell-task.delegations.asking': 'Asking',
  'shell-task.delegations.done': 'Done',
  'shell-task.delegations.failed': 'Failed',
  'shell-task.delegations.stopped': 'Stopped',
};
const t = (key: string): string => STRINGS[key] ?? key;
const harnessName = (id: string): string => ({ claude: 'Claude Code', codex: 'Codex' }[id] ?? id);

test('delegationsOf joins turn delegations with live child session rows', () => {
  const turn: TurnDoc = {
    id: 't1',
    idx: 0,
    status: 'running',
    ext: {
      'wisp.runtime': {
        delegations: [
          { name: 'codex', sessionId: 'c1', agent: 'codex', model: 'gpt-5.4', startedAt: 1000 },
          { name: 'reviewer', sessionId: 'c2', agent: 'claude', model: 'sonnet', startedAt: 1100 },
          { name: 'devbox-codex', sessionId: 'devbox|c3', agent: 'codex', startedAt: 1200, place: 'Devbox' },
        ],
      },
    },
  };
  const sessions = [
    { id: 'local:c1', state: 'running', agent: 'codex', summary: { model: 'gpt-5.4', live: 'Running tests', startedAt: 1050 } },
    { id: 'local:c2', state: 'done', agent: 'claude', summary: { model: 'sonnet', preview: 'LGTM, 0 issues', startedAt: 1100, endedAt: 4100 } },
  ];
  const rows = delegationsOf(turn, sessions, harnessName, t);
  expect(rows).toEqual([
    {
      id: 'c1',
      name: 'codex',
      agent: 'codex',
      line: 'Codex · gpt-5.4 — Running tests',
      status: 'Working',
      tone: 'accent',
      live: true,
      running: true,
      openable: true,
      startedAt: 1050,
    },
    {
      id: 'c2',
      name: 'reviewer',
      agent: 'claude',
      line: 'Claude Code · sonnet — LGTM, 0 issues',
      status: 'Done',
      tone: 'good',
      live: false,
      running: false,
      openable: true,
      startedAt: 1100,
      endedAt: 4100,
    },
    {
      id: 'devbox|c3',
      name: 'devbox-codex',
      agent: 'codex',
      line: 'Codex',
      status: 'Working',
      tone: 'accent',
      live: true,
      running: true,
      openable: false,
      place: 'Devbox',
      startedAt: 1200,
    },
  ]);
  expect(delegationTime(rows[0]!, 3050)).toBe(2000);
  expect(delegationTime(rows[1]!, 9000)).toBe(3000);
});

test('delegationsOf deduplicates repeated sessionIds and returns empty when none exist', () => {
  const empty: TurnDoc = { id: 't0', idx: 0, status: 'done' };
  expect(delegationsOf(empty, [], harnessName, t)).toEqual([]);

  const dup: TurnDoc = {
    id: 't1',
    idx: 0,
    status: 'done',
    ext: {
      'wisp.runtime': {
        delegations: [
          { name: 'codex', sessionId: 'c1', agent: 'codex', startedAt: 1000 },
          { name: 'codex', sessionId: 'c1', agent: 'codex', startedAt: 2000 },
        ],
      },
    },
  };
  expect(delegationsOf(dup, [], harnessName, t)).toHaveLength(1);
});
