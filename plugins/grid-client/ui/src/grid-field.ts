// What Settings → Cloud's grid address reads before you type in it (setup audit F2): the grid of the sign-in under way
// (or just declined), else the grid you're signed in to (Add an account), else wisp Cloud. Never quietly the hosted
// grid after you chose your own. Pure, so its test runs without a DOM.
import { DEFAULT_GRID, type CloudRow } from './commands.ts';

/** A grid's address as the field shows it: `grid.acme.dev`, or `http://…` kept for a plain-http one. */
const shown = (url: string): string => (url.startsWith('http://') ? url.replace(/\/$/, '') : url.replace(/^https:\/\//, '').replace(/\/$/, ''));

export function gridField(r: CloudRow | null | undefined): string {
  const flow = r?.signin?.grid ?? '';
  if (flow) return shown(flow);
  if (r?.signedIn && r.grid) return shown(r.grid);
  return DEFAULT_GRID;
}
