// wisp.shell-composer's UI: the queued follow-ups inside a task's reply box over its text (`shell.task:composer-top`,
// v1 .c-queued), first, before the ones set for a time, with v1's controls (send now, edit, combine, move, cancel). The composer's other parts (pickers, mentions, slash,
// the launcher model) are its library for the screens that host a composer.
import { lazy } from 'solid-js';
import type { PluginUiDef } from '@wisp/shell';
import catalogue from './i18n/en.json' with { type: 'json' };

const ui: PluginUiDef = {
  id: 'wisp.shell-composer',
  catalogue,
  // the @ and / menus of every prompt box (the shell draws them): loaded as the first screen draws (ADR-0067)
  later: () => import('./prompt.ts').then((m) => ({ default: { prompt: m.PROMPT_SOURCES } })),
  // v1 composer.js: `[...queuedItems(s), ...followUpItems(s)]`: the queue first (order 10), then wisp.later's (order 20)
  extensions: [{ id: 'shell-composer.queue', attachTo: { id: 'shell.task:composer-top' }, order: 10, component: lazy(() => import('./ui/QueueFoot.tsx')) }],
};
export default ui;
