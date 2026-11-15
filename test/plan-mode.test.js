// Plan mode (server/runner.js planRuns, adapters/claude.js planCommands): a task you start in Plan
// runs any command and only can't edit files; one a channel or trigger started stays read-only.

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import net from 'node:net';
import { installFakeBins, makeRepo, records, tmp, until } from './helpers.js';

const RECORD = tmp('plan-record');
process.env.FAKE_RECORD = RECORD;
process.env.WISP_HOME = tmp('plan-home');
process.env.WISP_UPDATE_CHECK = '0';
installFakeBins();

const { startServer } = await import('../server/index.js');
const { adapters } = await import('../server/adapters/index.js');
const { EDIT_TOOLS } = await import('../server/adapters/claude.js');
const { outsideTask, PLAN_NOTE } = await import('../server/runner.js');

let srv;
let base;
const freePort = () =>
  new Promise((resolve) => {
    const s = net.createServer().listen(0, '127.0.0.1', () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
  });
async function api(p, body) {
  const r = await fetch(`${base}/api${p}`, { method: body ? 'POST' : 'GET', headers: body ? { 'content-type': 'application/json' } : {}, body: body ? JSON.stringify(body) : undefined });
  const json = await r.json();
  if (r.status >= 400) throw new Error(`${p}: ${json.error}`);
  return json;
}
const runs = () => records(RECORD, 'claude').filter((r) => !r.stdin.startsWith('# wisp retro'));
const flag = (r, f) => r.argv[r.argv.indexOf(f) + 1];
const after_ = (r, f) => r.argv.slice(r.argv.indexOf(f) + 1, r.argv.indexOf('--verbose'));

before(async () => {
  const port = await freePort();
  base = `http://127.0.0.1:${port}`;
  srv = await startServer({ port, handleSignals: false });
});
after(() => {
  srv.shutdown();
  srv.server.closeAllConnections?.();
  srv.server.close();
});

test('Claude Code in Plan: with planCommands no prompts and the edit tools off; without, its own read-only plan mode', async () => {
  const argv = async (opts) => {
    const n = runs().length;
    await adapters.claude.run({ prompt: 'x', cwd: makeRepo(), mode: 'plan', systemPrompt: 'n', mcpServers: {}, onEvent: () => {}, ...opts }).done;
    await until(() => runs().length > n);
    return runs().at(-1);
  };
  const strict = await argv({});
  assert.equal(flag(strict, '--permission-mode'), 'plan');
  assert.ok(!strict.argv.includes('--disallowedTools'));
  const open = await argv({ planCommands: true, delegating: true });
  assert.equal(flag(open, '--permission-mode'), 'bypassPermissions');
  assert.deepEqual(after_(open, '--disallowedTools'), ['Task', 'Agent', 'Workflow', ...EDIT_TOOLS]);
  const gated = await argv({ planCommands: true, gate: true });
  assert.equal(flag(gated, '--permission-mode'), 'default', 'a side discussion’s gate wins');
  const edits = await argv({ planCommands: true, mode: 'edits' });
  assert.equal(flag(edits, '--permission-mode'), 'acceptEdits', 'only Plan');
});

test('a Plan task you start runs commands, with the edit tools off and the Plan note', async () => {
  const s = await api('/sessions', { prompt: 'how do I run the app?', cwd: makeRepo(), agent: 'claude', mode: 'plan' });
  await until(async () => (await api(`/sessions/${s.id}`)).state === 'review');
  const r = runs().at(-1);
  assert.equal(flag(r, '--permission-mode'), 'bypassPermissions', 'Claude Code’s plan mode refuses commands with side effects');
  assert.deepEqual(after_(r, '--disallowedTools').filter((t) => EDIT_TOOLS.includes(t)), EDIT_TOOLS);
  assert.ok(flag(r, '--append-system-prompt').includes(PLAN_NOTE.trim().slice(0, 40)));
  assert.match(flag(r, '--append-system-prompt'), /# Plan mode[\s\S]*don't change the project's files[\s\S]*run commands/);
});

test('Edits and Full auto keep their edit tools and get no Plan note', async () => {
  const s = await api('/sessions', { prompt: 'fix it', cwd: makeRepo(), agent: 'claude', mode: 'auto' });
  await until(async () => (await api(`/sessions/${s.id}`)).state === 'review');
  const r = runs().at(-1);
  assert.equal(flag(r, '--permission-mode'), 'bypassPermissions');
  assert.ok(!r.argv.includes('--disallowedTools'));
  assert.doesNotMatch(flag(r, '--append-system-prompt'), /# Plan mode/);
});

test('outsideTask: a channel’s, a trigger’s or a capped task (and their subagents) stays read-only in Plan', () => {
  const store = new Map();
  const get = { get: (id) => store.get(id) };
  assert.equal(outsideTask({ origin: null }, get), false, 'yours, from the page');
  assert.equal(outsideTask({ origin: { kind: 'recipe' } }, get), false);
  assert.equal(outsideTask({ origin: { kind: 'task', id: 'a' } }, get), false, 'one an agent of yours started');
  assert.equal(outsideTask({ origin: { kind: 'later', id: 'p' } }, get), false, 'your schedule or draft');
  assert.equal(outsideTask({ origin: { kind: 'trigger', id: 't' } }, get), true);
  assert.equal(outsideTask({ origin: { kind: 'github', name: 'x' } }, get), true);
  assert.equal(outsideTask({ capped: { mode: 'plan' } }, get), true);
  store.set('lead', { id: 'lead', origin: { kind: 'slack' } });
  assert.equal(outsideTask({ parentId: 'lead' }, get), true, 'a subagent goes by its lead');
});
