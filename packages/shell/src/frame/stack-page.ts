// F-phone-7: what a page shows, by layout (v1 body.split / body.stack / body.stack-page). Split (≥ 900 px): the list
// beside the main pane, Home in it. Stack (< 900 px, a phone): one page at a time: the list; or a task (a view, a
// crew member) as its own page with Back; or New task's prompt box (Home) over the list, which stays laid out
// behind it. While the list is the page, Home isn't built at all (F-phone-8: no hidden prompt box to tear down on a
// tap, nor to build on Back). Pure: App renders what this says.

import type { Nav } from '@wisp/client';
import type { StagePage } from '@wisp/ui';

export interface StackView {
  /** the page in front (split shows both; Stage reads it only in a stack) */
  readonly page: StagePage;
  /** a stack's main page over the list, which stays laid out behind it (New task from the list) */
  readonly listBehind: boolean;
  /** Home isn't on screen: the main pane builds nothing for it */
  readonly idleHome: boolean;
  /** the list's New task / Home row shows as the open one */
  readonly homeOn: boolean;
}

/** `launcher`: New task opened from the list (a stack's Home page). */
export function stackView(nav: Pick<Nav, 'layout' | 'main'>, launcher: boolean): StackView {
  const home = nav.main.kind === 'home';
  const page: StagePage = home && !launcher ? 'list' : 'main';
  return { page, listBehind: home, idleHome: nav.layout === 'stack' && page === 'list', homeOn: home && (nav.layout === 'split' || launcher) };
}

/** Esc on a stack's New task page (v1 keys.js stackPage → goBack): back to the list, once out of the text box (the
 *  first Esc blurs it, as everywhere) and when nothing over the page took the key. */
export function escLeavesLauncher(nav: Pick<Nav, 'layout' | 'main' | 'overlays'>, launcher: boolean, e: { readonly key: string; readonly defaultPrevented: boolean; readonly typing: boolean }): boolean {
  return e.key === 'Escape' && !e.defaultPrevented && !e.typing && launcher && nav.layout === 'stack' && nav.main.kind === 'home' && nav.overlays.length === 0;
}
