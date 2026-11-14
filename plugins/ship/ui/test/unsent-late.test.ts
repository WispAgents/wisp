// Ship and Merge count what waits to go with a task (comments not sent yet) through the shell's prompt sources. The
// comments' source is in shell-task's later part (ADR-0067): one that registers after Ship's dialog has set up still
// counts (a reload that restores the dialog can race it).
import { expect, test, vi } from 'vitest';
import { createRoot, createSignal } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import type { PromptSourceDef } from '@wisp/shell';

const [sources, setSources] = createSignal<readonly (PromptSourceDef & { plugin?: string })[]>([]);
const clients: Record<string, PluginClient> = { 'wisp.shell-task': { id: 'shell-task' } as unknown as PluginClient };
vi.mock('@wisp/shell', () => ({ useShell: () => ({ promptSources: () => sources }), usePluginClient: (p: string) => clients[p] }));
const { useUnsent } = await import('../src/unsent.ts');

test('a_waiting_source_that_registers_after_the_dialog_set_up_still_counts', () => {
  createRoot((dispose) => {
    const count = useUnsent();
    expect(count('p:a')).toBe(0);
    setSources([{ id: 'shell-task.comments', trigger: '/', items: () => [], plugin: 'wisp.shell-task', waiting: ({ client, task }) => (client.id === 'shell-task' && task === 'p:a' ? 2 : 0) }]);
    expect(count('p:a')).toBe(2);
    expect(count('p:b')).toBe(0);
    dispose();
  });
});
