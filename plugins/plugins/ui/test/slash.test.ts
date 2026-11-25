// `/plugin <ask>` in a prompt box casts a plugin as Settings → Plugins' box does; `/plugin` alone opens that pane.
import { expect, test, vi } from 'vitest';
import type { PluginClient } from '@wisp/client';
import later from '../src/later.ts';
import { pluginAsk, pluginItems } from '../src/slash.ts';

test('plugin_ask_reads_the_words_after_slash_plugin_and_nothing_else', () => {
  expect(pluginAsk('/plugin a button that opens my PRs')).toBe('a button that opens my PRs');
  expect(pluginAsk('  /Plugin\n  two\nlines ')).toBe('two\nlines');
  expect(pluginAsk('/plugin')).toBe('');
  expect(pluginAsk('/plugins on')).toBeNull();
  expect(pluginAsk('make a /plugin')).toBeNull();
});

test('the_slash_menu_offers_plugin_while_what_is_typed_could_still_be_it', () => {
  const say = (k: string): string => k;
  expect(pluginItems('', say).map((i) => i.insert)).toEqual(['/plugin ']);
  expect(pluginItems('PLU', say)).toHaveLength(1);
  expect(pluginItems('pluginx', say)).toEqual([]);
  expect(pluginItems('goal', say)).toEqual([]);
});

function client(run: (cmd: { id: string }, args: Record<string, unknown>) => Promise<unknown>) {
  const navigate = vi.fn();
  return { c: { home: 'me', run, navigate } as unknown as PluginClient, navigate };
}
const source = later.prompt?.[0];

test('slash_plugin_starts_a_cast_task_with_new_tasks_agent_and_opens_it', async () => {
  const run = vi.fn((_cmd: { id: string }, _args: Record<string, unknown>) => Promise.resolve({ id: 't1' }));
  const { c, navigate } = client(run);
  const say = vi.fn();
  expect(source?.send?.('/plugin dark sidebar', { client: c, task: '', launch: { agent: 'codex', model: 'gpt-5' }, say })).toBe(true);
  expect(say).toHaveBeenCalledOnce();
  const [cmd, args] = run.mock.calls[0] ?? [];
  expect(cmd?.id).toBe('runtime.create');
  expect(args).toMatchObject({ title: 'dark sidebar', agent: 'codex', model: 'gpt-5', ext: { 'wisp.plugins': { kind: 'cast' } } });
  expect(String(args?.['prompt'])).toContain('Make a wisp plugin that does this: dark sidebar');
  await Promise.resolve();
  await Promise.resolve();
  expect(navigate).toHaveBeenCalledWith({ t: 'openTask', id: 't1' });
});

test('slash_plugin_with_auto_sends_no_agent_and_a_refusal_puts_the_text_back', async () => {
  const run = vi.fn((_cmd: { id: string }, _args: Record<string, unknown>) => Promise.reject(new Error('no')));
  const { c } = client(run);
  const restore = vi.fn();
  expect(source?.send?.('/plugin x', { client: c, task: 'k', launch: { agent: 'auto' }, restore })).toBe(true);
  expect(run.mock.calls[0]?.[1]).not.toHaveProperty('agent');
  await new Promise((r) => setTimeout(r, 0));
  expect(restore).toHaveBeenCalledWith('/plugin x');
});

test('bare_slash_plugin_opens_settings_plugins_and_other_text_is_not_taken', () => {
  const run = vi.fn((_cmd: { id: string }, _args: Record<string, unknown>) => Promise.resolve({}));
  const { c, navigate } = client(run);
  expect(source?.send?.('/plugin', { client: c, task: '' })).toBe(true);
  expect(navigate).toHaveBeenCalledWith({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet', params: { pane: 'plugins' } } });
  expect(source?.send?.('fix the login bug', { client: c, task: '' })).toBe(false);
  expect(run).not.toHaveBeenCalled();
});
