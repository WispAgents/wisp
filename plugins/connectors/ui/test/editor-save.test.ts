// QA setup audit F9 (H): editing a connector dropped the environment variables you added. The update sent its form
// nested (`{ id, body: {…} }`) while the server reads it flat beside the id (connectors-api UpdateArgs,
// `#[serde(flatten)] body`): the body was ignored, so nothing the dialog changed was saved.
import { expect, test } from 'vitest';
import { updateConnector } from '../src/commands.ts';
import { bodyOf, draftOf, pasteDrafts } from '../src/lib.ts';
import type { ConnectorView } from '../src/types.ts';

test('an_edit_sends_its_form_flat_beside_the_id_with_the_env_you_added', () => {
  const saved = { id: 'c1', name: 'qa', transport: 'local', command: 'node', args: ['s.js'], line: 'node s.js', url: '', env: [], headers: [] } as unknown as ConnectorView;
  const d = draftOf(saved);
  d.env = [...d.env, { key: 'QA_VAR', value: 'hello', set: false }];
  const wire = updateConnector.wire as (a: unknown) => Record<string, unknown>;
  const sent = wire({ peer: 'p', id: 'c1', ...bodyOf(d) });
  expect(sent).toEqual({ id: 'c1', name: 'qa', transport: 'local', command: 'node s.js', env: [{ key: 'QA_VAR', value: 'hello' }] });
  expect(sent).not.toHaveProperty('body');
});

// QA setup audit F10: a pasted config with several servers added only the first, without a word. The form takes the
// first; the rest are added with it, and the dialog names them before you add.
test('a_pasted_config_with_several_servers_adds_them_all', () => {
  const text = JSON.stringify({ mcpServers: { pasted: { command: 'node', args: ['p.js'], env: { K: 'v' } }, remote1: { url: 'https://mcp.example.com/mcp' }, broken: { nothing: true } } });
  const r = pasteDrafts(text, '');
  expect(r?.draft.name).toBe('pasted');
  expect(r?.draft.env).toEqual([{ key: 'K', value: 'v', set: false }]);
  expect(r?.more.map((d) => [d.name, d.transport, d.url])).toEqual([['remote1', 'remote', 'https://mcp.example.com/mcp']]);
  // one server with no name of its own keeps the name typed
  expect(pasteDrafts(JSON.stringify({ command: 'npx', args: ['x'] }), 'mine')?.draft.name).toBe('mine');
  expect(pasteDrafts('not json', 'mine')).toBeNull();
  // names as the server takes them ([a-z0-9_], as typed ones are tidied), each once
  const odd = pasteDrafts(JSON.stringify({ mcpServers: { 'Sequential-Thinking': { command: 'a' }, sequential_thinking: { command: 'b' }, Linear: { url: 'https://l.example' } } }), '');
  expect([odd?.draft.name, ...(odd?.more ?? []).map((d) => d.name)]).toEqual(['sequential_thinking', 'linear']);
});
