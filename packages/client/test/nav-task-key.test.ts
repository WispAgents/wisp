// Opening a task by the bare id its list row carries (owner, 2026-11-17: search, Home and the list each opened a blank
// page): navigation resolves it to the task's entity key, any peer's, else the home peer's.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createNavigation, memoryHistory } from '../src/nav-url.ts';
import { createScope } from '../src/scope.ts';
import type { SessionId } from '../src/types.ts';

const keys = ['box:t2', 'laptop:t1'];
const taskKey = (id: string): string | undefined => keys.find((k) => k.slice(k.indexOf(':') + 1) === id);

test('a_task_opened_by_its_bare_id_lands_on_its_entity_key', () => {
  const nav = createNavigation({ scope: createScope(), width: 1280, history: memoryHistory('/'), home: 'laptop', taskKey });
  nav.navigate({ t: 'openTask', id: 't2' as SessionId });
  assert.deepEqual(nav.nav().main, { kind: 'task', id: 'box:t2' });
  nav.navigate({ t: 'openTask', id: 'nope' as SessionId });
  assert.deepEqual(nav.nav().main, { kind: 'task', id: 'laptop:nope' }, 'unknown: the home peer');
  nav.navigate({ t: 'openTask', id: 'laptop:t1' as SessionId });
  assert.deepEqual(nav.nav().main, { kind: 'task', id: 'laptop:t1' }, 'a key stays as it is');
});
