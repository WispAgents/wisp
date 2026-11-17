// j/k step through the list and open each task beside it, as v1 did (web/js/features/keys.js): the keys stay on the
// list, so the next j or k steps again instead of landing in the reply box (TRY-26). A task opened any other way (a
// click, a link) still puts the caret in its reply box. `/` goes to the reply box from the list, as in v1.

/** when j or k last opened a task (ms); a holder, not a module `let` (tools/check `state`) */
const last = { at: Number.NEGATIVE_INFINITY };

export function markStepped(now: number = Date.now()): void {
  last.at = now;
}

/** whether the task opening now was opened by j or k (within the frame or two it takes to mount) */
export function openedByStep(now: number = Date.now()): boolean {
  return now - last.at < 500;
}
