// The phone bars' scroll rules (design/mobile.md §A.1), pure: the extended FAB shrinks to its icon while
// the list scrolls down and grows back on scroll up (M3 extended FAB), and a top app bar's large title
// (M3 medium, iOS large title) collapses to the one-line title once the page scrolls. Both have a slop
// so a finger's jitter never flickers them, and both come back at the very top.

/** The FAB's state between scroll events: whether it is extended, and the scroll offset the current
 * direction started from. */
export interface FabScroll {
  readonly extended: boolean;
  readonly anchor: number;
}

export const FAB_SLOP = 8;

export const FAB_START: FabScroll = { extended: true, anchor: 0 };

/** The FAB after the list scrolled to `y` (its scrollTop). */
export function fabOnScroll(prev: FabScroll, y: number, slop: number = FAB_SLOP): FabScroll {
  if (y <= 0) return prev.extended && prev.anchor === 0 ? prev : { extended: true, anchor: 0 };
  if (prev.extended) {
    if (y - prev.anchor > slop) return { extended: false, anchor: y };
    return y < prev.anchor ? { extended: true, anchor: y } : prev;
  }
  if (prev.anchor - y > slop) return { extended: true, anchor: y };
  return y > prev.anchor ? { extended: false, anchor: y } : prev;
}

/** Whether a bar's large title is collapsed at scroll offset `y`, given the large title's height: it
 * collapses past half of it and comes back only under a quarter, so resting near the line never flickers. */
export function barCollapsed(prev: boolean, y: number, largeHeight: number): boolean {
  if (largeHeight <= 0) return y > 0;
  return prev ? y > largeHeight / 4 : y > largeHeight / 2;
}
