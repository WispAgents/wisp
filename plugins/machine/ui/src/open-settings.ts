// Settings at a pane, kept apart from host.ts so the plugin's registration (index.ts) loads nothing else (initial JS
// budget, ADR-0011).
import type { ExtensionId, Intent } from '@wisp/client';

/**
 * Opens Settings at a pane. The shell hasn't named its Settings view yet: this asks for an overlay
 * `shell.settings` with `{ pane, filter? }` (in the report), in one place so it's one edit.
 */
export function openSettings(navigate: (i: Intent) => void, pane: string, filter?: string): void {
  navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as ExtensionId, params: filter ? { pane, filter } : { pane } } });
}
