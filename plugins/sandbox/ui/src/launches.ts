// The launches' rows as the first screen needs them (ADR-0047, the owner's 2026-11-25 report): a `/sandbox` launch is
// a row of its own from the first frame, on top of the task list while its sandbox comes up (glowing), and kept there
// with why if it couldn't start (Try again, Dismiss). Watched once the first screen is up; the list's slot shows
// only while there is one, so a list with none is v1's to the pixel.
import { createSignal } from 'solid-js';
import { effect, type PluginClient } from '@wisp/client';
import { afterFirstScreen } from '@wisp/ui';

export const LAUNCHES_TOPIC = 'plugin:wisp.sandbox:launches' as const;
export const LAUNCH = 'sandbox.launch';
export const SIGNIN = 'sandbox.signin';

export type LaunchStep = 'make' | 'wake' | 'copy' | 'start';

/** A `/sandbox` launch, until its task started there (then the task's own row). */
export interface LaunchRow {
  readonly id: string;
  readonly title: string;
  /** preparing (`step` says where), signin (Claude's sign-in for sandboxes first), failed (`error`), started */
  readonly state: 'preparing' | 'signin' | 'failed' | 'started';
  readonly step: LaunchStep | '';
  readonly error?: string | null;
  readonly at: number;
  readonly agent: string;
  readonly peer?: string | null;
  readonly task?: string | null;
}

/** The rows the list shows: a started one is its task's own row by then. */
export const shown = (rows: readonly LaunchRow[]): LaunchRow[] => rows.filter((r) => r.state !== 'started').sort((a, b) => b.at - a.at);

const [count, setCount] = createSignal(0);
const watched = new WeakSet<PluginClient>();

/** Whether a launch is there to show (the list's slot asks): the topic is watched from the first call on. */
export function anyLaunch(c: PluginClient): boolean {
  if (!watched.has(c)) {
    watched.add(c);
    afterFirstScreen(() => {
      c.watch(c.home, LAUNCHES_TOPIC, c.scope);
      const list = c.entities.list<LaunchRow>(LAUNCH);
      const stop = effect(() => {
        setCount(shown(list() as readonly LaunchRow[]).length);
      });
      c.scope.add(stop);
    });
  }
  return count() > 0;
}
