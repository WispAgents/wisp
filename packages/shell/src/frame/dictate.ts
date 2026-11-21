// The prompt's mic, in the Android app only (v1 dictate.js, F-comp-14): the phone's speech recognizer types what you
// say where the caret was, its partial results as they come, a space from the words around it. Tap again to stop.
// The box changed under it (sent, cleared, typed in): it stops and leaves the box as it is. One box listens at a time.
// The listening itself is dictate-run.ts, its own chunk, fetched where the platform can dictate.
import { createSignal, onCleanup } from 'solid-js';
import type { ShellContext } from '../contract.ts';

/** The box a mic types into: its text, setting it, and the textarea (for the caret). */
export interface MicBox {
  text(): string;
  set(v: string): void;
  el(): HTMLTextAreaElement | undefined;
}

export interface Mic {
  readonly on: () => boolean;
  readonly label: () => string;
  toggle(): void;
}

/** The mic for `box`, or none where the platform can't dictate (a browser, the desktop app, an older phone app). */
export function useMic(shell: ShellContext, box: MicBox, onStart?: () => void): Mic | undefined {
  if (!shell.platform.dictation) return undefined;
  const [on, setOn] = createSignal(false);
  // fetched now, so a tap finds it there
  const run = import('./dictate-run.ts');
  let stop: (() => void) | undefined;
  let gone = false;
  const done = (): void => {
    stop = undefined;
    setOn(false);
  };
  onCleanup(() => ((gone = true), stop?.()));
  return {
    on,
    label: () => shell.t(on() ? 'shell.mic.stop' : 'shell.mic.start'),
    toggle() {
      if (stop) return stop();
      if (on()) return; // still starting
      setOn(true); // at once: the recognizer takes a moment to open
      onStart?.();
      void run.then((m) => {
        if (gone || !on()) return;
        const s = m.listen(shell, box, done);
        // it may end before it returns (the recognizer failed at once): then there's nothing to stop
        if (on()) stop = s;
      }, done);
    },
  };
}
