// The shell's own pieces, registered like any plugin's (plan §6.0.3: built-ins get no private API):
// (the frame draws the brand and Search itself: a plugin overrides them on shell.topbar:brand/search) the command palette, the shortcuts sheet and the
// diagnostics inspector as overlay views, the Diagnostics settings pane (the theme is General's, as v1's), and, as its
// later part (builtins-later.ts, loaded once the first screen is drawn), the palette's app commands and task list and
// the keys the shell answers itself (v1 keys.js: N, Esc, ?, [, ⌃Tab); ⌘K and ⌘L are here, so the first press of
// either opens what it opens at once, never kept waiting for that part. Any plugin can override one by id or
// switch it off through `tree`.

import { lazy } from 'solid-js';
import type { ExtensionId, Json } from '@wisp/client';
import type { PluginUiDef, ShellContext } from './contract.ts';
import { defineView } from './define.ts';
import { parseCompare } from './frame/runs-small.ts';
import { openFeedback, parseFeedback } from './frame/feedback.ts';
import { rowMenuItems } from './frame/row-menu.ts';
import { firstKeys } from './frame/launcher.ts';

const Palette = lazy(() => import('./palette/Palette.tsx'));
const Shortcuts = lazy(() => import('./frame/Shortcuts.tsx'));
const Inspector = lazy(() => import('./devtools/Inspector.tsx'));

const overlay = (id: string): { t: 'openOverlay'; overlay: { kind: 'dialog'; id: ExtensionId; params: null } } => ({
  t: 'openOverlay',
  overlay: { kind: 'dialog', id: id as ExtensionId, params: null },
});
const none = (p: unknown): null | undefined => (p === null || p === undefined ? null : undefined);
/** The palette's params: nothing (⌘K), or the switcher's narrowing ({ only, placeholder, title }: ⌘P). */
const paletteParams = (p: unknown): Json | undefined => {
  if (p === null || p === undefined) return null;
  if (typeof p !== 'object' || Array.isArray(p)) return undefined;
  const o = p as { only?: unknown; placeholder?: unknown; title?: unknown };
  const only = Array.isArray(o.only) ? o.only.filter((x): x is string => typeof x === 'string') : undefined;
  return { ...(only && { only }), ...(typeof o.placeholder === 'string' && { placeholder: o.placeholder }), ...(typeof o.title === 'string' && { title: o.title }) };
};

export function shellUi(ctx: ShellContext): PluginUiDef {
  return {
    id: 'shell',
    // its entries in a task's row menu (v1 taskMenu), attached like any plugin's
    extensions: rowMenuItems(ctx),
    views: [
      defineView({
        kind: 'shell.palette',
        slot: 'overlay',
        overlay: 'dialog',
        bare: true,
        place: 'top',
        parse: paletteParams,
        component: Palette,
        title: (p) => (p as { title?: string } | null)?.title ?? ctx.t('shell.palette.title'),
      }),
      defineView({ kind: 'shell.shortcuts', slot: 'overlay', overlay: 'dialog', parse: none, component: Shortcuts, title: () => ctx.t('shell.shortcuts.title') }),
      // add, order, rename and remove projects (v1 features/projects.js); every "Add a project" opens it
      defineView({
        kind: 'shell.projects',
        slot: 'overlay',
        overlay: 'dialog',
        part: 'projects.modal',
        parse: none,
        component: lazy(() => import('./frame/Projects.tsx')),
        title: () => ctx.t('shell.projects.title'),
      }),
      // the runs of one ask side by side (v1 compare.js compareView); a task's Compare runs opens it
      // Rerun… (v1 rerunDialog): the task's spec to edit, then a new task from it
      defineView({
        kind: 'shell.rerun',
        slot: 'overlay',
        overlay: 'dialog',
        part: 'rerun.modal',
        parse: (p) => ({ id: String((p as { id?: unknown } | null)?.id ?? ''), ...(typeof (p as { agent?: unknown } | null)?.agent === 'string' && { agent: (p as { agent: string }).agent }) }),
        component: lazy(() => import('./frame/Rerun.tsx')),
        title: () => ctx.t('shell.rerun.title'),
      }),
      // Delete task, asked first: the head's ⋯, ⌫ and Delete on the open task or the picked row (audit A #4)
      defineView({
        kind: 'shell.delete',
        slot: 'overlay',
        overlay: 'dialog',
        part: 'task.delete.modal',
        parse: (p) => ({ id: String((p as { id?: unknown } | null)?.id ?? '') }),
        component: lazy(() => import('./frame/DeleteDialog.tsx')),
        title: () => ctx.t('shell.delete.title'),
      }),
      // Rename task: one dialog for the head, the rows and the phone (QA round 15 #2; v1 actions.js rename's rules)
      defineView({
        kind: 'shell.rename',
        slot: 'overlay',
        overlay: 'dialog',
        part: 'list.rename',
        parse: (p) => ({ id: String((p as { id?: unknown } | null)?.id ?? '') }),
        component: lazy(() => import('./frame/RenameDialog.tsx')),
        title: () => ctx.t('shell.rename.title'),
      }),
      // Fork… (v1 forkTask): what the fork should try, its mode and harness, then a copy carrying on from a turn; nothing
      // is sent before its Fork (QA round 14 desktop #1)
      defineView({
        kind: 'shell.fork',
        slot: 'overlay',
        overlay: 'dialog',
        part: 'fork.modal',
        parse: (p) => ({ id: String((p as { id?: unknown } | null)?.id ?? ''), ...(typeof (p as { turn?: unknown } | null)?.turn === 'string' && { turn: (p as { turn: string }).turn }) }),
        component: lazy(() => import('./frame/Fork.tsx')),
        title: () => ctx.t('shell.fork.title'),
      }),
      // Send feedback (v1 feedback.js openFeedback): ⌘K, Home's foot, `/feedback` with no words
      defineView({
        kind: 'shell.feedback',
        slot: 'overlay',
        overlay: 'dialog',
        part: 'feedback.modal',
        parse: parseFeedback,
        component: lazy(() => import('./frame/Feedback.tsx')),
        title: () => ctx.t('shell.feedback.title'),
      }),
      defineView({
        kind: 'shell.compare',
        slot: 'overlay',
        overlay: 'dialog',
        part: 'compare.modal',
        parse: parseCompare,
        component: lazy(() => import('./frame/Compare.tsx')),
        title: () => ctx.t('shell.compare.title'),
      }),
      // All goals… / All recipes…: every row of a menu that grows with the formula, searched (owner-list #63)
      defineView({
        kind: 'shell.pick',
        slot: 'overlay',
        overlay: 'dialog',
        bare: true,
        part: 'shell.pick',
        // its rows stay in ./frame/pick-list.ts (lazy) under this key
        parse: (p) => ({ key: String((p as { key?: unknown } | null)?.key ?? '') }),
        component: lazy(() => import('./frame/PickList.tsx')),
        title: () => ctx.t('shell.home.goalRow'),
      }),
      defineView({ kind: 'shell.devtools', slot: 'overlay', overlay: 'viewer', parse: none, component: Inspector, title: () => ctx.t('shell.diag.title') }),
    ],
    // the two keys a first press reaches for before anything else (the palette, New task): never waiting for `later`
    shortcuts: firstKeys(ctx),
    settings: [
      { id: 'shell.diagnostics', group: 'advanced', title: 'shell.diag.title', icon: 'bug', component: Inspector, order: 100 },
    ],
    // the palette's sources and the keys, once the first screen is drawn
    later: () => import('./builtins-later.ts').then((m) => ({ default: m.shellLater(ctx) })),
  };
}
