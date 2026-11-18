// The mic's listening (dictate.ts's engine, v1 dictate.js): what you say typed where the caret was, its partial results
// as they come, a space from the words around it; the box changed under it, it stops. Its own chunk: only the phone apps
// can dictate, so a browser's or the desktop's first page never carries it (budget, ADR-0011).
import '../i18n/lazy.ts';
import type { ShellContext } from '../contract.ts';
import type { DictationError } from '../platform/types.ts';
import type { MicBox } from './dictate.ts';

/** `words` said between `before` and `after`: the box's text and where the caret goes. */
export function spoken(before: string, after: string, words: string): { text: string; caret: number } {
  const w = words.trim();
  const lead = w && before && !/\s$/.test(before) ? ' ' : '';
  const tail = w && after && !/^\s/.test(after) ? ' ' : '';
  return { text: before + lead + w + tail + after, caret: (before + lead + w).length };
}

const QUIET: Readonly<Partial<Record<DictationError, true>>> = { silence: true };

/** Listens into `box` from its caret; `done` once it ends (or fails). Its stop. */
export function listen(shell: ShellContext, box: MicBox, done: () => void): () => void {
  const d = shell.platform.dictation;
  if (!d) return done(), () => undefined;
  const el = box.el();
  const v = box.text();
  const at = el?.selectionStart ?? v.length;
  let around = { before: v.slice(0, at), after: v.slice(el?.selectionEnd ?? at), last: v };
  let stop: (() => void) | undefined;
  const put = (words: string): void => {
    if (box.text() !== around.last) return stop?.(); // the box changed under it
    const s = spoken(around.before, around.after, words);
    around = { ...around, last: s.text };
    box.set(s.text);
    queueMicrotask(() => box.el()?.setSelectionRange(s.caret, s.caret));
  };
  stop = d.start((e) => {
    if (e.state === 'partial' || e.state === 'final') put(e.text);
    if (e.state === 'error') {
      if (e.text) put(e.text);
      const msg = shell.t(`shell.mic.${e.error}`);
      if (QUIET[e.error]) shell.toast.show(msg);
      else shell.toast.error(msg, { duration: e.error === 'blocked' ? 8000 : 4200 });
    }
    if (e.state === 'end' || e.state === 'error') done();
  });
  return () => stop?.();
}
