// Ship it's dialog title ("Ship it", "Push to PR #412"): the view's registration reads it for the overlay's title, so
// it is all of the dialog's words the first page loads (budget, ADR-0011); the rest (ship.ts) loads with the dialog.
import type { T } from '../i18n.ts';
import type { ShipGuess } from './ship.ts';

export function dialogTitle(t: T, g: ShipGuess): string {
  if (g.update) return g.repos > 1 ? t('wisp.ship.dialog.pushMany', { count: g.repos }) : t('wisp.ship.dialog.pushOne', { n: g.pr ?? 0 });
  return g.repos > 1 ? t('wisp.ship.dialog.shipMany', { count: g.repos }) : t('wisp.ship.dialog.ship');
}
