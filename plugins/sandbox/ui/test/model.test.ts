// ADR-0047: `/sandbox …` in New task, and a sandbox's owner said in words.
import { expect, test } from 'vitest';
import { codexEnded, codexFlow, envKeyProblem, liveList, ownerTask, ownerWords, sandboxPrompt, type Listed } from '../src/model.ts';

const say = (k: string, v?: Readonly<Record<string, string | number>>): string => (v ? `${k}(${Object.values(v).join(',')})` : k);

test('a_sandbox_prompt_is_read_off_new_task_and_its_owner_is_said_in_words', () => {
  expect(sandboxPrompt('/sandbox fix the login')).toBe('fix the login');
  expect(sandboxPrompt('  /Sandbox\ntwo lines\nhere')).toBe('two lines\nhere');
  expect(sandboxPrompt('/sandbox')).toBe('');
  expect(sandboxPrompt('fix the login')).toBeNull();
  expect(sandboxPrompt('/sandboxes')).toBeNull();
  expect(ownerWords({ task: 'p:t1' }, say)).toBe('sandbox.owner.task');
  expect(ownerWords({ lead: 't1', member: 'codex' }, say)).toBe('sandbox.owner.delegate(codex)');
  expect(ownerWords({ crew: 'ada', job: 'm|t' }, say, () => 'Ada')).toBe('sandbox.owner.job(Ada)');
  expect(ownerWords({ crew: 'ada' }, say, () => 'Ada')).toBe('sandbox.owner.member(Ada)');
  expect(ownerWords({ judges: 't1' }, say)).toBe('sandbox.owner.judges');
  expect(ownerWords(null, say)).toBe('sandbox.owner.none');
  expect([ownerTask({ task: 'p:t1' }), ownerTask({ crew: 'x' })]).toEqual(['p:t1', undefined]);
});

// SBX-A6: a name the grid's env_key_ok refuses was sent anyway, and its refusal dropped what was typed
test('an_env_var_name_is_checked_as_the_grid_checks_it_one_rule_each', () => {
  expect(envKeyProblem('')).toBe('empty');
  expect(envKeyProblem('A'.repeat(65))).toBe('long');
  expect(envKeyProblem('A'.repeat(64))).toBeUndefined();
  expect(envKeyProblem('1PASSWORD')).toBe('digit');
  expect(envKeyProblem('MY-VAR')).toBe('shape');
  expect(envKeyProblem('PATH')).toBe('reserved');
  expect(envKeyProblem('HOME')).toBe('reserved');
  expect(envKeyProblem('CLAUDE_CODE_OAUTH_TOKEN')).toBe('reserved');
  expect(envKeyProblem('WISP_TOKEN')).toBe('reserved');
  expect(envKeyProblem('LD_PRELOAD')).toBe('reserved');
  expect(envKeyProblem('DATABASE_URL')).toBeUndefined();
  expect(envKeyProblem('_PRIVATE')).toBeUndefined();
  expect(envKeyProblem('PATHS'), 'only the name itself').toBeUndefined();
});

// SBX-A5: the list follows the machine rows; SBX-A2: the grid's others after them; a deleted one stays gone
test('the_list_follows_the_machine_rows_and_keeps_the_grids_others_and_a_deleted_one_out', () => {
  const info: Listed[] = [
    { machine: 'm1', peer: 'm1', sandbox: 'aa', name: 'Sandbox web', owner: { task: 'p:t' }, state: 'online' },
    { machine: '', peer: '', sandbox: 'bb', name: 'api', owner: null, state: '', elsewhere: true },
  ];
  expect(liveList(info, [], [], new Set()).map((l) => l.sandbox), 'before the rows: sandbox.info').toEqual(['aa', 'bb']);
  const rows = [{ id: 'h:m2', body: { id: 'm2', name: 'Sandbox new', data: { sandbox: 'cc', owner: { pending: 1 } } } }, { id: 'h:mac', body: { id: 'mac', data: {} } }];
  const live = liveList(info, rows, [{ id: 'h:m2', body: { id: 'm2', state: 'asleep' } }], new Set());
  expect(live.map((l) => [l.sandbox, l.state])).toEqual([
    ['cc', 'asleep'],
    ['bb', ''],
  ]);
  expect(liveList(info, rows, [], new Set(['bb'])).map((l) => l.sandbox)).toEqual(['cc']);
  expect(liveList(info, [{ id: 'h:m3', body: { id: 'm3', data: { sandbox: 'bb' } } }], [], new Set()).map((l) => [l.sandbox, !!l.elsewhere])).toEqual([['bb', false]]);
});

// SBX-A3: accounts.login.start answers before the CLI printed its page: that's waiting, not nothing
test('codex_signing_in_there_is_waiting_until_its_page_comes_then_done_or_failed', () => {
  expect(codexFlow({ state: 'running' })).toEqual({ state: 'starting' });
  expect(codexFlow({ state: 'waiting', url: 'https://x', code: 'AB-12' })).toEqual({ state: 'link', url: 'https://x', code: 'AB-12' });
  expect(codexFlow({ state: 'done' })).toEqual({ state: 'done' });
  expect(codexFlow({ state: 'failed', error: 'no' })).toEqual({ state: 'failed', error: 'no' });
  expect([codexEnded({ state: 'starting' }), codexEnded({ state: 'done' }), codexEnded({ state: 'failed', error: '' })]).toEqual([false, true, true]);
});
