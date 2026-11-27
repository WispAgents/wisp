// New task's box, from anywhere (⌘L, N, the palette's New task): the open overlay closed, then the box focused, or
// Home when no screen shows one. And the two keys a first press reaches for before anything else (⌘K, ⌘L), which
// builtins.tsx registers at start so they never wait for the later part (builtins-later.ts: N and the palette row).
// Its own small file: the first page carries it.
import type { ExtensionId } from '@wisp/client';
import type { ShellContext, ShortcutDef } from '../contract.ts';

export function launcher(ctx: ShellContext): void {
  if (ctx.nav().overlays.length > 0) ctx.navigate({ t: 'closeOverlay', id: ctx.nav().overlays[0]?.id as ExtensionId });
  if (!ctx.focusTargets.focus('launcher')) ctx.navigate({ t: 'home' });
}

/** ⌘K (the palette) and ⌘L (New task), registered with the shell's first part. */
export function firstKeys(ctx: ShellContext): ShortcutDef[] {
  return [
    { id: 'shell.palette', keys: 'mod+k', label: 'shell.key.palette', group: 'general', when: 'always', run: () => ctx.palette.open() },
    { id: 'shell.launcher', keys: 'mod+l', label: 'shell.key.launcher', group: 'general', when: 'always', run: () => launcher(ctx) },
  ];
}
