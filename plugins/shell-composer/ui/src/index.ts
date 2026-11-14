// wisp.shell-composer's UI: the queued follow-ups over a task's reply box (`shell.task:foot`), with v1's
// controls (send now, edit, combine, move, cancel). The composer's other parts (pickers, mentions, slash,
// the launcher model) are its library for the screens that host a composer.
import { lazy } from 'solid-js';
import type { PluginUiDef } from '@wisp/shell';
import catalogue from './i18n/en.json' with { type: 'json' };

const ui: PluginUiDef = {
  id: 'wisp.shell-composer',
  catalogue,
  // the @ and / menus of every prompt box (the shell draws them): loaded as the first screen draws (ADR-0067)
  later: () => import('./prompt.ts').then((m) => ({ default: { prompt: m.PROMPT_SOURCES } })),
  extensions: [{ id: 'shell-composer.queue', attachTo: { id: 'shell.task:foot' }, component: lazy(() => import('./ui/QueueFoot.tsx')) }],
};
export default ui;
