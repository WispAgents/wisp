// The way back to the Grid from a cloud wisp (docs/ux/2026-11-18-grid.md #9), DOM-free: in a grid box, its row names
// the grid's own address, and the account menu links there (your machines, keys and links); the Home tip that puts a
// computer on a grid is offered only where this wisp isn't already on one.
import type { CloudRow } from './commands.ts';

const BARE = /^https?:\/\/[a-z0-9][a-z0-9.:-]*$/i;

/** The grid's Home to link back to, from this wisp's row: only a box's own, well-formed address. */
export function gridHome(row: CloudRow | undefined): string | undefined {
  const h = row?.home?.trim() ?? '';
  return BARE.test(h) ? `${h}/` : undefined;
}

/** "Reach this wisp from anywhere" once the row says where this wisp runs: never in a grid box, nor before it lands
 *  (a box would show it for a moment, then take it back). */
export function offerReach(row: CloudRow | undefined): boolean {
  return row !== undefined && !gridHome(row);
}
