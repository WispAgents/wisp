// This computer on its grid, as Settings → Machines and Settings → Cloud say it: its state and, once it's on, its
// address on the grid (TRY-17: the row said only "Online", never where). Pure, so the UI test imports it without a DOM.
import type { Computer } from './commands.ts';
import type { Key, Vars } from './i18n.ts';

export interface Here {
  /** off, connecting, online, trying again (its error) */
  readonly line: string;
  /** where it's reached on the grid (`https://studio--ada.grid.trywisp.dev`), shown as a link; none while it's off */
  readonly url?: string;
}

export function hereOf(k: Computer | null | undefined, t: (key: Key, vars?: Vars) => string): Here {
  if (!k?.on) return { line: t('grid-client.computer.off') };
  const url = /^https?:\/\/[^\s]+$/.test(k.url) ? { url: k.url } : {};
  if (k.state === 'online') return { line: t('grid-client.computer.onlineNoUrl'), ...url };
  if (k.state === 'retrying') return { line: k.error || t('grid-client.computer.retrying'), ...url };
  return { line: t('grid-client.computer.connecting'), ...url };
}
