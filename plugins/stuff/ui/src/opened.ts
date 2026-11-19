// Two small rules My Stuff's page follows, DOM-free (the tests read them without the page's components).
import { reachable } from './preview/spec.ts';

/** A press (a click, a key) opened the page just now: the browser still holds the user's activation from it. A reload or a
 *  link restored it: no activation. Engines without `userActivation` count as opened (v1 focused on open). */
export function openedByPress(nav: { userActivation?: { isActive: boolean } } = navigator as never): boolean {
  return nav.userActivation ? nav.userActivation.isActive : true;
}

/**
 * Where an app's page loads from this window (v1 launchApp: `isLoopback(location.href) ? url : boxUrl(url, preview)`):
 * as served when this page is on the computer; else at the grid's preview address when a plugin maps it (ADR-0064:
 * wisp.grid-client, inside a box, for this computer's own app); else nowhere (undefined).
 */
export function appAddress(url: string, page: string, maps: readonly ((url: string) => string | undefined)[]): string | undefined {
  const here = reachable(url, page, null);
  if (here) return here;
  for (const map of maps) {
    const to = map(url);
    if (to) return to;
  }
  return undefined;
}

/** Open shows it inside wisp (the reader, the viewer) where the screen can; the phone screen's apps go over the whole
 *  screen instead (`launched`), and a screen with no `show` opens a new window (audit C #1). */
export function opensHere(kind: string, d: { readonly show?: unknown; readonly launched?: unknown }): boolean {
  return !!d.show && !(kind === 'app' && d.launched);
}
