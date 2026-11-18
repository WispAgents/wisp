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
