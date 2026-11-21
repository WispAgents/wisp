// F-phone-8: the task page's code comes down while the list sits there, not on the first tap. On a phone on
// Slow 4G the first tap waited 1.2 s on its chunks (TaskView, then each plugin's part of the page, then each
// turn's) with nothing on screen. Once the list has painted and the page is idle, every lazy component that
// draws an open task (the screen, the task's and the turn's inputs) is asked to preload. Code only: no data,
// and nothing a plugin didn't already say it would show there. Pure: the scheduling is the caller's.

import { INPUTS } from '../contract.ts';

/** The inputs an open task draws: its screen and every `shell.task:*` / `shell.turn:*` input. */
export const TASK_PAGE_INPUTS: readonly string[] = ['shell.screen:task', ...Object.keys(INPUTS).filter((k) => k.startsWith('shell.task:') || k.startsWith('shell.turn:'))];

type Preloadable = { readonly preload?: () => Promise<unknown> };

/** The preloads to run: each lazy component once (a component on two inputs loads once), plain ones skipped. */
export function taskPagePreloads(components: readonly unknown[]): (() => Promise<unknown>)[] {
  const seen = new Set<unknown>();
  const out: (() => Promise<unknown>)[] = [];
  for (const c of components) {
    const p = (c as Preloadable | null | undefined)?.preload;
    if (typeof p !== 'function' || seen.has(c)) continue;
    seen.add(c);
    out.push(() => p().catch(() => undefined)); // offline or a bad chunk: the tap loads it again, as before
  }
  return out;
}
