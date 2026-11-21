// Whether a touch starts in a sideways scroller: apart from the binders (bind.ts) so a row's own touch handler
// (Swipe.tsx) can ask without loading them (ADR-0105).
import { scrollsX } from './gesture-math.ts';

/** The kit's horizontal scrollers, marked so (a box overflowing with overflow-x auto/scroll counts too). */
const SCROLLER = "[data-kit='code-block'], [data-kit='diff'], [data-scroll-x]";

/** Whether `target`, or a box between it and `root`, scrolls sideways (one style read per box, on touch start only). */
export function inScroller(target: EventTarget | null, root: Element): boolean {
  for (let el = target instanceof Element ? target : null; el && el !== root; el = el.parentElement) {
    if (el.matches(SCROLLER)) return true;
    const cs = getComputedStyle(el);
    if (scrollsX({ overflowX: cs.overflowX, scrollWidth: el.scrollWidth, clientWidth: el.clientWidth, marked: false })) return true;
  }
  return false;
}
