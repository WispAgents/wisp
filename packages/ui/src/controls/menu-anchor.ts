// Where a menu opens from (MenuProps.below): the trigger's box stretched down to the bottom of its ancestor `selector`,
// so a menu placed under it opens below that box, at the trigger's side (QA desktop r3 #5: Delegate's Presets under
// the panel's head, not over its words or the launcher's bar). Zag reads it as `positioning.getAnchorRect`.

export const underBox =
  (selector: string) =>
  (el: unknown): { x: number; y: number; width: number; height: number } | null => {
    if (!(el instanceof HTMLElement)) return null;
    const r = el.getBoundingClientRect();
    const box = el.closest(selector)?.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: Math.max(r.height, (box?.bottom ?? r.bottom) - r.y) };
  };

type AnchorRect = { x: number; y: number; width: number; height: number };

/** A point to open from while `point` gives one (a long-press's finger, QA r8 #2), else the trigger's own box (or what
 *  `fallback` makes of it). */
export const fromPoint =
  (point: () => { readonly x: number; readonly y: number } | undefined, fallback?: (el: unknown) => AnchorRect | null) =>
  (el: unknown): AnchorRect | null => {
    const at = point();
    if (at) return { x: at.x, y: at.y, width: 0, height: 0 };
    if (fallback) return fallback(el);
    if (!(el instanceof HTMLElement)) return null;
    const r = el.getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  };
