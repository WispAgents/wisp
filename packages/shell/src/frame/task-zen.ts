// Zen mode for one task (⌃.): its chat shows the messages and the answer only — no tool calls, thinking or
// in-between steps — whatever Settings → Chat says for every other task. Kept per task in this browser, as its place
// is (place.ts), so going back or a reload keeps it; the oldest go first past MAX_ZEN.
import type { Json, PluginClient, PluginStore } from '@wisp/client';
import type { ChatSettings } from './chat-settings.ts';

export const MAX_ZEN = 100;

/** The zen tasks with `id` turned on (newest last) or off, at most MAX_ZEN. */
export function zenSet(all: readonly string[], id: string, on: boolean): string[] {
  const rest = all.filter((k) => k !== id);
  return (on ? [...rest, id] : rest).slice(-MAX_ZEN);
}

/** The persisted value, read defensively: a list of task keys. */
export const asZenList = (v: unknown): readonly string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : []);

/** The task's chat settings with zen on top: Chat mode, and tool calls 'zen'; the rest as Settings says. */
export function withZen(base: ChatSettings, zen: () => boolean): ChatSettings {
  return {
    ...base,
    chatMode: () => zen() || base.chatMode(),
    toolCalls: () => (zen() ? 'zen' : base.toolCalls()),
  };
}

const stores = new WeakMap<object, PluginStore<Json>>();

/** The zen tasks, in this browser: the shell's client's store. */
export function zenStore(root: { plugin(id: string): PluginClient }): PluginStore<Json> {
  const had = stores.get(root);
  if (had) return had;
  const store = root.plugin('shell').defineStore<Json>({ id: 'shell.zen', version: 1, defaults: [], persist: true, check: (v) => (Array.isArray(v) ? (v as Json) : undefined) });
  stores.set(root, store);
  return store;
}
