// wisp.shell-home's UI: the finish / ask notifier (v1 notify.js), started by apps/web with the shell's
// platform, toaster and nav (a plugin UI's setup gets only its client); its strings; and Home's pure rules
// (what needs you, overlapping tasks, the tips) for the screens that show them.
import type { PluginUiDef } from '@wisp/shell';
import catalogue from './i18n/en.json' with { type: 'json' };

import type { NotifierDeps } from './notify.ts';

export type { NotifierDeps } from './notify.ts';

/** Starts the notifier (./notify.ts), which loads apart from the first page (ADR-0011): it watches what changes from
 * then on, so a few milliseconds later misses nothing. Returns its stop. */
export function startNotifier(d: NotifierDeps): () => void {
  let stop: (() => void) | undefined;
  let stopped = false;
  void import('./notify.ts').then(
    (m) => {
      if (!stopped) stop = m.startNotifier(d);
    },
    () => undefined,
  );
  return () => {
    stopped = true;
    stop?.();
  };
}

const ui: PluginUiDef = { id: 'wisp.shell-home', catalogue };
export default ui;
