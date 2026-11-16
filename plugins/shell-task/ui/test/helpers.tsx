// Test helpers: the task view on the real @wisp/client and the mock peer (dev/), mounted in
// happy-dom with a ShellContext and the task's TaskProvider.
import { afterEach } from 'vitest';
import { render } from 'solid-js/web';
import type { JSX } from 'solid-js';
import { entityKey, peerId, type EntityKey } from '@wisp/client';
import type { Topic } from '@wisp/protocol';
import { createToaster, KitProvider, type Toaster } from '@wisp/ui';
import { harness, type Harness, type HarnessOptions } from '../dev/harness.ts';
import { devShell } from '../dev/shell-context.ts';
import { ShellProvider } from '../src/shell.ts';
import { TaskProvider, createTaskContext, type TaskContext } from '../src/task-context.tsx';

const mounted: Array<() => void> = [];
afterEach(() => {
  while (mounted.length) mounted.pop()?.();
  document.body.innerHTML = '';
});

/** until `ok`, checked after each macrotask (what the test then asserts, never a fixed count of timer hops: on Windows
 * each is a ~15.6 ms system tick); gives up after `ms` by the clock, and the assertion after it fails with its message */
export async function until(ok: () => boolean, ms = 4000): Promise<void> {
  const end = performance.now() + ms;
  while (!ok() && performance.now() < end) await new Promise((r) => setTimeout(r, 0));
}
export const part = (name: string): HTMLElement | null => document.querySelector<HTMLElement>(`[data-part="${name}"]`);
export const parts = (name: string): HTMLElement[] => [...document.querySelectorAll<HTMLElement>(`[data-part="${name}"]`)];

export interface Mounted {
  readonly h: Harness;
  readonly key: EntityKey;
  readonly toaster: Toaster;
  readonly task: () => TaskContext;
  readonly host: HTMLElement;
}

/**
 * Mounts `view` inside the open task `sessionId` (its rows come from `o.rows`, keyed by topic).
 * Resolves once the task's topics are live; what the view then draws, the test waits for (`until`).
 */
export async function mountTask(o: Omit<HarnessOptions, 'plugin'> & { sessionId: string }, view: () => JSX.Element): Promise<Mounted> {
  const topics = o.watch ?? (Object.keys(o.rows) as Topic[]);
  const h = harness({ ...o, plugin: 'wisp.shell-task', watch: topics });
  await h.ready();
  const key = entityKey(peerId(o.peer ?? 'laptop'), o.sessionId);
  const toaster = createToaster();
  const shell = devShell(h.client, toaster);
  let ctx: TaskContext | undefined;
  const host = document.createElement('div');
  document.body.append(host);
  mounted.push(
    render(
      () => (
        <KitProvider t={(k, v) => shell.t(k, v)}>
          <ShellProvider value={shell}>
            {(() => {
              ctx = createTaskContext(h.plugin, key);
              return <TaskProvider value={ctx}>{view()}</TaskProvider>;
            })()}
          </ShellProvider>
        </KitProvider>
      ),
      host,
    ),
  );
  mounted.push(() => h.client.dispose());
  return { h, key, toaster, host, task: () => ctx as TaskContext };
}
