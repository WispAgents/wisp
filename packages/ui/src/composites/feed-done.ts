// The feed list's Mark done column (ListRow's feed layout): a plain module, so a test reads it without the components.
import type { CssVars } from '../core/props.ts';

/** The feed list's Mark done column on a touch screen (v1 split.css `#feed.has-quick`): set on the list (its `vars`)
 *  while some row in it shows Mark done. On touch (hover: none) Save for later leaves the row and Mark done takes a
 *  36px column of its own after the time, never over the title; with no row showing it there is no column. With a
 *  mouse the vars do nothing (the quick actions float over the title's end while they show). */
export function feedDoneColumn(on: boolean): CssVars {
  // -x: the column and the row's 10px column gap before it, where the pills end (the time keeps to the same edge)
  return on ? { '--k-feed-done': '36px', '--k-feed-done-pad': '48px', '--k-feed-done-x': '46px' } : {};
}
