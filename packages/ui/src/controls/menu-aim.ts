// The safe triangle of a cascading menu, pure: from where the pointer left a submenu's row to the near edge of its
// flyout. While the pointer moves inside it, it is on its way to the flyout, so the flyout stays open even as it
// crosses the rows in between (owner 2026-11-17). Zag draws the same triangle but closes 100 ms after the row is left,
// shorter than a real diagonal takes: ./menu-flyout.tsx holds the flyout open with this one instead.

export interface Point {
  readonly x: number;
  readonly y: number;
}

export interface Box {
  readonly left: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
}

export type Triangle = readonly [Point, Point, Point];

/** The triangle from `from` to the flyout's near edge: its left edge when it opened to the right, else its right. */
export function aimTriangle(from: Point, fly: Box): Triangle {
  const x = fly.left >= from.x ? fly.left : fly.right;
  return [from, { x, y: fly.top }, { x, y: fly.bottom }];
}

const cross = (o: Point, a: Point, b: Point): number => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);

/** Inside the triangle or on its edge (either winding). */
export function inTriangle(p: Point, [a, b, c]: Triangle): boolean {
  const d1 = cross(a, b, p);
  const d2 = cross(b, c, p);
  const d3 = cross(c, a, p);
  const neg = d1 < 0 || d2 < 0 || d3 < 0;
  const pos = d1 > 0 || d2 > 0 || d3 > 0;
  return !(neg && pos);
}

export function inBox(p: Point, b: Box): boolean {
  return p.x >= b.left && p.x <= b.right && p.y >= b.top && p.y <= b.bottom;
}

/** A box of zero size (not laid out): nothing can aim at it. */
export const emptyBox = (b: Box): boolean => b.right <= b.left || b.bottom <= b.top;
