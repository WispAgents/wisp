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

/** wisp's modified keys with no browser default worth keeping: pressed early, kept for the later part (see dispatch) */
const KEPT_MODIFIED: readonly string[] = ['mod+k', 'mod+l', 'ctrl+`'];

export function isTyping(target: EventTarget | null): boolean {
  const el = target as Element | null;
  return typeof el?.closest === 'function' && el.closest(TYPING) !== null;
}

/** an Enter or Space on a button, link or other control: the control's own press */
function onControl(e: KeyboardEvent): boolean {
  const el = e.target as Element | null;
  return (e.key === 'Enter' || e.key === ' ') && typeof el?.closest === 'function' && el.closest('button, a[href], [role="button"], [role="menuitem"], [role="option"], summary') !== null;
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
    // did nothing). Never kept: a key typed into a box, held down, an Enter or Space on a control (it presses that),
    // and a modified one (⌘F, ⌘P, ⌘1…): the browser's own find, print or tab switch runs instead, as before the keys
    // were wisp's (kept, it ran twice; kept and prevented, an early ⌘F with no task open did nothing at all). Nothing
    // starts early or loads again for a kept key (laterPending), and once the parts are in nothing is kept.
    // Except the modified keys whose browser default nobody wants here (⌘K, ⌘L: the address bar; ⌃`): kept and
    // prevented, as they would be once the parts are in (final6 F2: an early ⌘K did nothing at all).
    const ours = KEPT_MODIFIED.some((k) => matches(k, e, ctx.platform.apple));
    if (e.repeat || (!ours && (e.metaKey || e.ctrlKey || e.altKey)) || isTyping(e.target) || onControl(e)) return;
    if (!ours && ![...BUILT_IN_KEYS].some((k) => matches(k, e, ctx.platform.apple))) return;
    const wait = ctx.laterPending?.();
    // nothing on its way: its default is left alone, as a kept key's is (the replay may not run)
    if (!wait) return;
    if (ours) e.preventDefault();
    const where = (): string => {
      const nav = ctx.nav();
      return JSON.stringify([nav.main, nav.overlays.length, ctx.selection.get() ?? null]);
    };
    const at = where();
    void wait.then(() => {
      if (where() === at) run(e);
    }, () => undefined);
  };
  w.addEventListener('keydown', guard, true);
  w.addEventListener('keydown', dispatch);
  return () => {
    w.removeEventListener('keydown', guard, true);
    w.removeEventListener('keydown', dispatch);
  };
}
