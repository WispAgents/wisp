// The page-wide key listener: the IME guard in the capture phase (before any text box sees the
// Enter that only picked a word), then the shortcut dispatch once the event has bubbled past
// whatever had focus. A popover or a dialog that handled Escape itself (Zag does) called
// preventDefault, so its Escape never also goes Back.

import type { PluginClient } from '@wisp/client';
import type { ShellContext } from './contract.ts';
import { BUILT_IN_KEYS, candidates, imeArtifact, matches } from './keys.ts';
import type { Registry } from './registry.ts';
import type { ShellWindow } from './shell.tsx';

// a title renamed in place is `plaintext-only` (audit A review: ⌫ there opened Delete, E marked the task done)
const TYPING = 'input, textarea, select, [contenteditable="true"], [contenteditable=""], [contenteditable="plaintext-only"]';

export function isTyping(target: EventTarget | null): boolean {
  const el = target as Element | null;
  return typeof el?.closest === 'function' && el.closest(TYPING) !== null;
}

export function installKeys(w: ShellWindow, ctx: ShellContext, registry: Registry, clientOf: (plugin: string) => PluginClient): () => void {
  const guard = (e: Event): void => {
    if (imeArtifact(e as KeyboardEvent)) e.stopImmediatePropagation();
  };
  const run = (e: KeyboardEvent): boolean => {
    const nav = ctx.nav();
    const st = { typing: isTyping(e.target), overlay: nav.overlays.length > 0, focused: ctx.selection.get(), apple: ctx.platform.apple };
    for (const s of candidates(registry.shortcuts(), e, st)) {
      if (s.run({ client: clientOf(s.plugin), nav, focused: st.focused, event: e }) !== false) return true;
    }
    return false;
  };
  const dispatch = (ev: Event): void => {
    const e = ev as KeyboardEvent;
    if (e.defaultPrevented || e.repeat && e.key === 'Escape') return;
    if (run(e)) return void e.preventDefault();
    // one of wisp's own keys pressed while its later part (builtins-later.ts: the task and row keys) is still on its
    // way, in the page's first moments: it runs once that lands, where the press found it (⇧F right after a task opens
    // did nothing). A key typed into a box or held down is never kept.
    if (e.repeat || isTyping(e.target) || ![...BUILT_IN_KEYS].some((k) => matches(k, e, ctx.platform.apple))) return;
    const wait = ctx.laterReady?.();
    // its default is left alone (an Enter on a focused button still presses it)
    if (!wait) return;
    const at = { main: ctx.nav().main, focused: ctx.selection.get() };
    void wait.then(() => {
      if (ctx.nav().main === at.main && ctx.selection.get() === at.focused) run(e);
    }, () => undefined);
  };
  w.addEventListener('keydown', guard, true);
  w.addEventListener('keydown', dispatch);
  return () => {
    w.removeEventListener('keydown', guard, true);
    w.removeEventListener('keydown', dispatch);
  };
}
