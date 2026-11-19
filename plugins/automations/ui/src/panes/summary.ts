// The Recipes / Triggers line of Settings → Automations, a plain module so its test needn't compile the pane.
import { t } from '../i18n.ts';
import '../i18n/lazy.ts';

/** "None yet · save a prompt you run often", or the names (the first three, then how many more). */
export function summary(names: readonly string[], none: string): string {
  if (names.length === 0) return `${t('automations.none')} · ${none}`;
  const shown = names.slice(0, 3).join(', ');
  return names.length > 3 ? t('automations.summary.more', { names: shown, n: names.length - 3 }) : shown;
}

/** The recipes by their names as written (`title`, a v1 recipe's "Run tests"), else their keys: what the launcher's chip and
 *  Manage say (audit E4). */
export const recipeNames = (rs: readonly { readonly name: string; readonly title?: string | undefined }[]): string[] => rs.map((r) => r.title?.trim() || r.name);

/** A trigger's last run in words (v1 agoLong): "last run just now", never "last run now ago" (audit B #16). */
export function lastRunWords(at: number | null | undefined, now: number, ago: (at: number) => string, say: (k: string, v?: Readonly<Record<string, string>>) => string): string {
  if (!at) return say('automations.page.notRun');
  return now - at < 45_000 ? say('automations.page.lastRunNow') : say('automations.page.lastRun', { ago: ago(at) });
}
