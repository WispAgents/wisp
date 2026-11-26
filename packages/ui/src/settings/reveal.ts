// Settings' search lands on the setting, not only on its pane (doc924a): the row whose `settingKey` was found is
// scrolled to the middle, lit for a moment and its control focused, so the next key press changes it. A pane that
// builds its rows later (All settings reads them from the server, then draws them over frames) is watched until the
// row comes or `wait` runs out; a row that never comes does nothing. Its timers and watch are the caller's
// component's: they end with it.
import { onCleanup } from 'solid-js';
import { useTimeout } from '../blocks/rich-scope.ts';

/** How long a found row stays lit (the CSS fades it, or not under reduced motion). */
export const FOUND_MS = 1500;

const CONTROL = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** The row for `key` under `root`, by the `data-setting-key` SettingsRow puts on it. */
export function settingRow(root: ParentNode, key: string): HTMLElement | null {
  return root.querySelector<HTMLElement>(`[data-setting-key="${key.replace(/["\\]/g, '\\$&')}"]`);
}

/** `reveal(root, key)` finds `key`'s row under `root` (now, or once it's built, for up to `wait` ms), scrolls to it,
 *  lights it and focuses its control; a new reveal or `stop` ends the last one and puts its light out. */
export function useRevealSetting(wait = 3000): { readonly reveal: (root: ParentNode & Node, key: string) => void; readonly stop: () => void } {
  const out = useTimeout();
  const giveUp = useTimeout();
  let lit: HTMLElement | undefined;
  let watch: MutationObserver | undefined;
  const stopWatching = (): void => {
    watch?.disconnect();
    watch = undefined;
    giveUp.stop();
  };
  const stop = (): void => {
    stopWatching();
    out.stop();
    lit?.removeAttribute('data-found');
    lit = undefined;
  };
  onCleanup(stop);
  const light = (row: HTMLElement): void => {
    stopWatching();
    lit = row;
    row.scrollIntoView?.({ block: 'center' });
    row.setAttribute('data-found', '');
    // its own control first (a hint's link comes before it in the row), else anything in the row that takes focus
    const control = row.querySelector<HTMLElement>(`[data-kit='settings-row-control'] :is(${CONTROL})`) ?? row.querySelector<HTMLElement>(CONTROL);
    control?.focus({ preventScroll: true });
    out.start(FOUND_MS, () => row.removeAttribute('data-found'));
  };
  const reveal = (root: ParentNode & Node, key: string): void => {
    stop();
    const now = settingRow(root, key);
    if (now) return light(now);
    if (typeof MutationObserver !== 'function') return;
    watch = new MutationObserver(() => {
      const row = settingRow(root, key);
      if (row) light(row);
    });
    watch.observe(root, { childList: true, subtree: true });
    giveUp.start(wait, stopWatching);
  };
  return { reveal, stop };
}
