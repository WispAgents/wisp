// @wisp-plugin/shell-task-ui: the open task (v1 split view's task page and side panel). The task
// view and the side panel load on first show (lazy), so the shell pays nothing until a task opens.
import { lazy } from 'solid-js';

const TaskSide = lazy(() => import('./TaskSide.tsx'));
import { defineExtension, definePluginUi, defineView } from './define.ts';
import { TAB_VIEW, TERM_VIEW, WATCH_VIEW, isTui } from './open-task.ts';
import { CATALOGUE } from './i18n/index.ts';
import { CONTINUE_VIEW, parseContinueParams } from './continue/model.ts';

export { CATALOGUE };
export type * from './types.ts';

export default definePluginUi({
  id: 'wisp.shell-task',
  catalogue: CATALOGUE as Record<string, string>,
  // The frame's task view is the chassis (its head renames in place, its transcript folds); this plugin fills
  // its slots: the side panel (Changes, PR, Shell), the cards under each turn, and what waits over the reply box.
  // the panel's head (its tabs) shows on every side area, so switching panes keeps it
  extensions: [
    ...(['changes', 'pr', 'shell', 'preview'] as const).map((p) => defineExtension({ id: `shell-task.side.${p}`, attachTo: { id: `shell.side:${p}` }, component: TaskSide })),
    // under each turn: why it stopped, what holds it, what moved in the CLI, its receipt
    defineExtension({ id: 'shell-task.turn', attachTo: { id: 'shell.turn:foot' }, component: lazy(() => import('./cards/TurnFoot.tsx')) }),
    // over a turn that switched harness: the Timeline's divider (ADR-0124)
    defineExtension({ id: 'shell-task.switch', attachTo: { id: 'shell.turn:head' }, component: lazy(() => import('./continue/SwitchDivider.tsx')) }),
    // in each turn's trail, after its token count: the receipt's cache and plan share (ADR-0095)
    defineExtension({ id: 'shell-task.receipt', attachTo: { id: 'shell.turn:trail' }, component: lazy(() => import('./cards/TrailReceipt.tsx')) }),
    // over the reply box: the agent's question (or a tool's approval) and the queued follow-ups
    defineExtension({ id: 'shell-task.foot', attachTo: { id: 'shell.task:foot' }, component: lazy(() => import('./foot/TaskFoot.tsx')) }),
    // over the reply box: the comments on lines waiting for the next follow-up (v1 comments.js tray)
    defineExtension({ id: 'shell-task.comments', attachTo: { id: 'shell.task:foot' }, component: lazy(() => import('./foot/CommentTray.tsx')) }),
    // the agent opened a watch while you look at its task: the Watch tab opens beside it (v1 panel.js)
    defineExtension({ id: 'shell-task.watch-open', attachTo: { id: 'shell.task:foot' }, component: lazy(() => import('./foot/WatchOpen.tsx')) }),
    // a terminal task's CLI is its body, edge to edge under its head (v1 .tui-term)
    defineExtension({ id: 'shell-task.term-screen', attachTo: { id: 'shell.task:screen' }, if: (p) => isTui(p.session?.()), component: lazy(() => import('./foot/TermScreen.tsx')) }),
  ],
  // Run app and Watch in a task's row menu, the comments' prompt source and Open file…: loaded as the first screen
  // draws (ADR-0067)
  later: () => import('./later.ts'),
  views: [
    // Continue on…: the runner picker, what carries over, one follow-up on that harness (ADR-0124)
    defineView({ kind: CONTINUE_VIEW, slot: 'overlay', overlay: 'dialog', parse: parseContinueParams, title: () => CATALOGUE['shell-task.continue.title'] ?? '', component: lazy(() => import('./continue/ContinueOn.tsx')) as never }),
    defineView({ kind: WATCH_VIEW, slot: 'side', parse: () => null, title: () => CATALOGUE['shell-task.side.watch'] ?? '', component: TaskSide as never }),
    defineView({ kind: TERM_VIEW, slot: 'side', parse: () => null, title: () => CATALOGUE['shell-task.side.term'] ?? '', component: TaskSide as never }),
    defineView({ kind: TAB_VIEW, slot: 'side', parse: () => null, title: () => CATALOGUE['shell-task.side.label'] ?? '', component: TaskSide as never }),
  ],
});
