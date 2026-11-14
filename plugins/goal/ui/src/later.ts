// wisp.goal's UI, the part only a press reaches (ADR-0067: PluginUiDef.later): loaded as the first screen draws.
import type { EntityKey } from '@wisp/client';
import type { PluginUiLater } from '@wisp/shell';
import { defineExtension } from './define.ts';
import { t } from './i18n.ts';
import { ACTION_ICON, actionLabel, runGoalAction } from './actions.ts';
import { goalActions, type GoalAction } from './rules.ts';
import type { SessionRow } from './types.ts';
import { sendGoal } from './slash.ts';

// the goal's entries in a task's row menu (v1 goal.js goalMenuHtml): Set goal… (Edit goal… once there is one),
// Goal checks…, Pause or Resume goal, Drop goal, each only where it applies
const rowOf = (p: { session?: () => unknown }): SessionRow | undefined => p.session?.() as SessionRow | undefined;
const rowItem = (a: GoalAction, order: number, id: string) =>
  defineExtension({
    id,
    attachTo: { id: 'shell.row:menu' },
    component: () => null,
    order,
    if: (p) => goalActions(rowOf(p)).includes(a),
    bar: {
      icon: ACTION_ICON[a],
      label: actionLabel(a, undefined),
      priority: order,
      sub: 'goal',
      text: (p) => t(actionLabel(a, rowOf(p))),
      select: (p) => {
        const s = rowOf(p);
        if (s) runGoalAction(p.client, a, s.id as EntityKey);
      },
    },
  });

const later: PluginUiLater = {
  extensions: [
    rowItem('set', 110, 'wisp.goal.rowSet'),
    rowItem('checks', 111, 'wisp.goal.rowChecks'),
    rowItem('pause', 112, 'wisp.goal.rowPause'),
    rowItem('resume', 112, 'wisp.goal.rowResume'),
    rowItem('drop', 113, 'wisp.goal.rowDrop'),
  ],
  // `/goal …` in a task's reply box: the task's goal, never a message for its agent (New task's is the server's)
  prompt: [{ id: 'wisp.goal.slash', trigger: '/', where: ['reply'], order: 80, items: () => [], send: (text, ctx) => sendGoal(text, ctx, Date.now()) }],
};
export default later;
