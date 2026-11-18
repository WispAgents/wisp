// The row's third line (F-task-6, v1 feed.js runLine): harness, the model's label, where it ran; the tip names the
// model ("Default model" when none was picked), the effort as "<x> thinking" and the place as "on <place>".
import { expect, test } from 'vitest';
import { pillsOf, runOf, type RowContext, variantOf } from '../src/model/row.ts';
import type { SessionRow } from '../src/types.ts';
import { flatSession } from '../src/model/wire.ts';

const row = (o: Partial<SessionRow>): SessionRow => ({ id: 's1', title: 'Fix login', agent: 'claude', state: 'review', createdAt: 0, updatedAt: 0, ...o }) as SessionRow;
const agents = new Map([['claude', { id: 'claude', name: 'Claude Code', models: [{ id: 'opus', label: 'Opus 4.7' }] }]]) as unknown as RowContext['agents'];
const c: RowContext = { split: true, open: false, swipes: false, here: 'This computer', agents };

test('a_rows_run_line_says_harness_model_label_and_place_with_v1s_tip', () => {
  const r = runOf(row({ model: 'opus', effort: 'high' } as Partial<SessionRow>), c);
  expect(r?.agentName).toBe('Claude Code');
  expect(r?.model).toBe('Opus 4.7');
  expect(r?.where).toBe('This computer');
  expect(r?.title).toEqual({ key: 'shell-list.run.tip-effort', vars: { agent: 'Claude Code', model: 'Opus 4.7', effort: 'High', where: 'This computer' } });
  const bare = runOf(row({ machine: { id: 'm1', name: 'box-1' } } as Partial<SessionRow>), c);
  expect(bare?.model).toBeUndefined();
  expect(bare?.where).toBe('box-1');
  expect(bare?.remote).toBe(true);
  expect(bare?.title.key).toBe('shell-list.run.tip-default');
  // no run line for wisp's own rows and reminders
  expect(runOf(row({ agent: 'wisp' }), c)).toBeUndefined();
  expect(runOf(row({ kind: 'reminder' } as Partial<SessionRow>), c)).toBeUndefined();
});

test('a_crashed_limited_or_refused_rows_pill_says_which_v1_halt', async () => {
  // F-task-14: the summary's `halt` (runtime core summarize) beats the agent's "Blocked"; a finished verdict keeps its own word
  const label = (o: Record<string, unknown>): unknown => pillsOf(flatSession({ id: 's1', state: 'review', summary: { status: 'blocked', ...o } } as never)).find((p) => p.id === 'status')?.label;
  expect(label({ halt: 'limit' })).toEqual({ key: 'shell-list.status.limit' });
  expect(label({ halt: 'refused' })).toEqual({ key: 'shell-list.status.refused' });
  expect(label({ halt: 'failed' })).toEqual({ key: 'shell-list.status.failed' });
  expect(label({})).toEqual({ key: 'shell-list.status.blocked' });
  expect(label({ status: 'done', halt: 'failed' })).toEqual({ key: 'shell-list.status.done' });
});

test('a_capped_rows_pill_names_its_cap_and_an_uncapped_row_has_none', async () => {
  // F-chan-10: runtime's list field `capped`, flattened from the summary like every other
  const cap = (o: Record<string, unknown>): unknown => pillsOf(flatSession({ id: 's1', state: 'review', summary: { status: 'done', ...o } } as never)).find((p) => p.id === 'capped');
  expect(cap({ capped: 'edits' })).toEqual({ id: 'capped', label: { key: 'shell-list.pill.capped-edits' }, tone: 'warn', icon: 'lock', variant: 'outline', title: { key: 'shell-list.pill.capped-edits-tip' } });
  expect(cap({ capped: 'plan' })).toMatchObject({ label: { key: 'shell-list.pill.capped-plan' } });
  expect(cap({})).toBeUndefined();
  expect(cap({ capped: 'auto' })).toBeUndefined();
});

test('a_trial_runs_line_says_its_one_change_after_the_model_v1_try_two_ways', async () => {
  // F-comp-31: runtime's `variant`, flattened from the summary; both rows of a trial share a harness, this tells them apart
  const b = flatSession({ id: 's2', state: 'review', agent: 'claude', summary: { status: 'done', variant: { subagentModel: 'haiku', skills: 'off' } } } as never);
  expect(variantOf(b)).toEqual([
    { key: 'shell-list.run.subagents', vars: { name: 'Haiku' } },
    { key: 'shell-list.run.skills-off' },
  ]);
  expect(runOf(b, c)?.variant).toHaveLength(2);
  expect(variantOf(row({ variant: { subagentModel: '' } } as Partial<SessionRow>))).toEqual([{ key: 'shell-list.run.subagents-task' }]);
  expect(runOf(row({}), c)?.variant).toBeUndefined();
});

// One name everywhere (owner, after QA r10 #2): a task on its harness's Default names the model that runs in the tip, as
// its chip does (ShellContext.defaultModel through the row context); the line itself stays as v1 drew it (no model)
test('a_row_on_the_default_model_names_it_in_the_tip_as_the_chip_does', () => {
  const asked: string[] = [];
  const withDefault: RowContext = { ...c, defaultModel: (s) => (asked.push(`${s.agent} ${s.cwd ?? '-'}`), s.agent === 'claude' ? 'Opus' : undefined) };
  const r = runOf(row({ cwd: '/code/app' } as Partial<SessionRow>), withDefault);
  expect(r?.model).toBeUndefined();
  expect(r?.title).toEqual({ key: 'shell-list.run.tip', vars: { agent: 'Claude Code', model: 'Opus', effort: '', where: 'This computer' } });
  expect(asked).toEqual(['claude /code/app']);
  const e = runOf(row({ effort: 'high' } as Partial<SessionRow>), withDefault);
  expect(e?.title.key).toBe('shell-list.run.tip-effort');
  // a picked model is never asked about; one the store can't tell keeps "Default model"
  runOf(row({ model: 'opus' } as Partial<SessionRow>), withDefault);
  expect(asked).toHaveLength(2);
  expect(runOf(row({ agent: 'codex' }), withDefault)?.title.key).toBe('shell-list.run.tip-default');
});

// QA r15 #3: a model the harness's offers don't list still reads in words (a Claude id), never the id
test('a_rows_model_reads_in_words_when_the_offers_dont_name_it', () => {
  expect(runOf(row({ model: 'claude-opus-5-5' } as Partial<SessionRow>), c)?.model).toBe('Opus 5.5');
});
