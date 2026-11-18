// wisp.budget's UI: Settings → Usage (the week's spend and outcomes, each plan's windows, where unattended work stops),
// the Usage view (charts and breakdowns), and on Home Spare allowance and usage at a glance.
import { lazy } from 'solid-js';
import { defineExtension, defineView, type PluginUiDef } from '@wisp/shell';
import { VIEW_USAGE } from './views.ts';
import { catalogue, t } from './i18n.ts';

const ui: PluginUiDef = {
  id: 'wisp.budget',
  settings: [{ id: 'usage', group: 'agents', title: 'budget.usage.pane', icon: 'stats', order: 20, component: lazy(() => import('./panes/Usage.tsx')) }],
  views: [defineView({ kind: VIEW_USAGE, slot: 'overlay', overlay: 'dialog', parse: () => null, title: () => t('budget.view.title'), component: lazy(() => import('./UsageView.tsx')) })],
  // Spare allowance and window changes on Home, where v1 had them (.sh-spare)
  // (UX 2026-11-18 desktop #8: no usage bar under Home's prompt box any more: the harness chip's ring and menu and
  // Settings → Usage show it; HomeUsage stays for a later home input)
  extensions: [
    defineExtension({ id: 'budget.spare', attachTo: { id: 'shell.home:cards' }, component: lazy(() => import('./Spare.tsx')) }),
  ],
  // Ask near the limit in the prompt boxes (v1 hold.js): asked before New task and a follow-up go
  // (its code and words load on the first send: ADR-0067); `budget_hold` is the server-side guard's refusal reason
  sendHolds: [{ reason: 'budget_hold', check: ({ client }, a) => import('./hold.ts').then((m) => m.checkHold(client, a)) }],
  catalogue,
};
export default ui;
