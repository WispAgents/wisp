// `/plugin <what it should do>` in New task or a task's reply box: Settings → Plugins' Cast from where you type.
// Pure: what the box's text asks for, and the / menu's row.
import type { PromptItem } from '@wisp/shell';
import './i18n/lazy.ts';

/** `/plugin <ask>` → the ask ('' when nothing came after it), or null when the text isn't one. `/plugins` is not it. */
export function pluginAsk(text: string): string | null {
  const m = /^\/plugin(?:\s+([\s\S]*))?$/i.exec(text.trim());
  return m ? (m[1] ?? '').trim() : null;
}

/** The / menu's row while what's typed could still be `/plugin` (`/`, `/p`, `/plug`). */
export function pluginItems(query: string, say: (k: string) => string): PromptItem[] {
  if (!'plugin'.startsWith(query.toLowerCase())) return [];
  return [{ value: '/plugin', label: '/plugin', detail: say('plugins.slash.args'), hint: say('plugins.slash.hint'), icon: 'plug', insert: '/plugin ' }];
}
