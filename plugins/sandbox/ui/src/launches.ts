// The launches' rows as the first screen needs them (ADR-0047, the owner's 2026-11-25 report): a `/sandbox` launch is
// a row of its own from the first frame, on top of the task list while its sandbox comes up (glowing), and kept there
// with why if it couldn't start (Try again, Dismiss). Watched once the first screen is up; the list's slot shows
// only while there is one, so a list with none is v1's to the pixel.
import { createSignal } from 'solid-js';
import { effect, type PluginClient } from '@wisp/client';
import { afterFirstScreen } from '@wisp/ui';
import { shown, type LaunchRow } from './launch-model.ts';

export { shown, stepsOf, type LaunchRow, type LaunchStep } from './launch-model.ts';

export const LAUNCHES_TOPIC = 'plugin:wisp.sandbox:launches' as const;
export const LAUNCH = 'sandbox.launch';

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
