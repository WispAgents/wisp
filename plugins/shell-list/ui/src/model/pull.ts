// Pull to refresh's pending state (ADR-0017: the pull answers within one frame). `reconnect()` returns nothing and a
// peer's status may flip to `connecting` a tick later (or never, when every peer is already online), so the spinner is
// not a same-tick read of the status: it turns on the moment the pull lands and stays on until the reconnect settles,
// seen as the status going `connecting` and leaving it again (online, or failed with its error in the offline bar).
// If nothing goes `connecting` within `brief` ms (the connection was healthy, a no-op), it clears after that brief spin.
import { createEffect, createSignal, onCleanup } from 'solid-js';

/** one-shot timer (the list's `scope.timeout`, so disposing the list clears it); returns its cancel */
export type After = (ms: number, run: () => void) => () => void;

export const PULL_BRIEF_MS = 600;

export function createPull(connecting: () => boolean, reconnect: () => void, after: After, brief = PULL_BRIEF_MS): { readonly pull: () => void; readonly refreshing: () => boolean } {
  const [pulled, setPulled] = createSignal(false);
  let seen = false; // this pull has watched a peer go `connecting`
  let briefDone = false;
  let cancel: (() => void) | undefined;
  const settle = (): void => {
    cancel?.();
    cancel = undefined;
    setPulled(false);
  };
  createEffect(() => {
    const c = connecting();
    if (!pulled()) return;
    if (c) seen = true;
    else if (seen && briefDone) settle();
  });
  onCleanup(() => cancel?.());
  return {
    pull: () => {
      cancel?.();
      seen = connecting();
      briefDone = false;
      setPulled(true);
      reconnect();
      cancel = after(brief, () => {
        briefDone = true;
        if (!seen || !connecting()) settle();
      });
    },
    refreshing: pulled,
  };
}
