// What waits to go with a task's next follow-up (v1 F-page-11: "2 comments not sent yet" before Ship or Merge), read
// through the shell's prompt sources (`PromptSourceDef.waiting`): the comments are shell-task's drafts, kept in its
// own client store, so Ship never reads that store; it asks each source that answers, with that source's own client.
import { createMemo } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { useShell, usePluginClient } from '@wisp/shell';

/** Counts what every source holds for the task, when called. Call the hook in a component's setup. */
export function useUnsent(): (task: string) => number {
  const shell = useShell();
  const sources = shell.promptSources?.() ?? (() => []);
  // over the sources as they are, not as they were at setup: one that registers later (a plugin's `later` part,
  // ADR-0067) counts too. The memo is owned by the component, so its context still answers usePluginClient.
  const clients = createMemo(() => {
    const out = new Map<string, PluginClient>();
    for (const p of sources()) {
      const plugin = (p as { plugin?: string }).plugin ?? 'shell';
      if (p.waiting && !out.has(plugin)) out.set(plugin, usePluginClient(plugin));
    }
    return out;
  });
  return (task) =>
    sources().reduce((n, p) => {
      const client = clients().get((p as { plugin?: string }).plugin ?? 'shell');
      return n + (p.waiting && client ? p.waiting({ client, task }) : 0);
    }, 0);
}

/** The row as Ship reads it, with what waits counted in. */
export const withUnsent = <S extends { readonly id: string; readonly unsentComments?: number }>(
  s: S | undefined,
  count: (task: string) => number,
): (S & { readonly unsentComments: number }) | undefined =>
  s ? { ...s, unsentComments: (s.unsentComments ?? 0) + count(s.id) } : s;
