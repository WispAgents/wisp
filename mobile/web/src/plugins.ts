// Every plugin UI package in this checkout (plugins/*/ui), as apps/web/src/plugins.ts. In its own module so the entry's
// `import '@wisp/ui/styles'` runs first: Vite hoists an eager glob's imports above the module's own, and a plugin's kit
// sheet arriving before index.css would declare `@layer kit` before `reset` (the reset's `button` rule then beat every
// kit control: no button backgrounds, no switch track, a see-through FAB).
import type { PluginUiDef } from '@wisp/shell';

const found = import.meta.glob<{ default?: PluginUiDef }>(['../../../plugins/*/ui/src/index.ts', '../../../plugins/*/ui/src/index.tsx'], { eager: true });

export function pluginUis(): { path: string; def: PluginUiDef | undefined }[] {
  return Object.entries(found)
    .map(([path, mod]) => ({ path, def: mod.default }))
    .sort((a, b) => a.path.localeCompare(b.path));
}
