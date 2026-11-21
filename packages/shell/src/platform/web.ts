// The web's own answers to the platform calls ADR-0065 added, shared by every implementation: the safe-area
// insets the page's CSS sees (env(safe-area-inset-*)), the window's size class and a foldable's hinge (the
// Viewport Segments API). The phone apps' WebViews answer these too; their native sides add nothing yet.

import type { BackEvent, Insets, PlatformWindow, Rect, SizeClass, WindowLayout } from './types.ts';

const ZERO: Insets = { top: 0, right: 0, bottom: 0, left: 0 };

/** M3 window size classes (design/mobile.md §A.3): compact under 600, medium under 840, expanded from 840. */
export function sizeClass(width: number): SizeClass {
  return width < 600 ? 'compact' : width < 840 ? 'medium' : 'expanded';
}

/** What the system draws over the page's edges, read once from a probe with env() padding. */
export function webSafeArea(w: PlatformWindow): Insets {
  const doc = w.document;
  const body = doc.body;
  if (!body || typeof w.getComputedStyle !== 'function') return ZERO;
  const probe = doc.createElement('div');
  // custom properties don't resolve env(); padding does. Fixed and hidden: it takes no room and paints nothing.
  probe.style.cssText = 'position:fixed;visibility:hidden;pointer-events:none;top:0;left:0;padding:env(safe-area-inset-top,0px) env(safe-area-inset-right,0px) env(safe-area-inset-bottom,0px) env(safe-area-inset-left,0px)';
  body.appendChild(probe);
  const cs = w.getComputedStyle(probe);
  const px = (v: string): number => {
    const n = Number.parseFloat(v);
    return Number.isFinite(n) && n > 0 ? n : 0;
  };
  const out = { top: px(cs.paddingTop), right: px(cs.paddingRight), bottom: px(cs.paddingBottom), left: px(cs.paddingLeft) };
  probe.remove();
  return out;
}

/** The gap between a foldable's two segments (side by side or stacked); undefined on one screen. */
export function hingeOf(segments: readonly Rect[] | null | undefined): Rect | undefined {
  if (!segments || segments.length !== 2) return undefined;
  const [a, b] = segments;
  if (!a || !b) return undefined;
  if (b.x >= a.x + a.width) return { x: a.x + a.width, y: Math.min(a.y, b.y), width: b.x - (a.x + a.width), height: Math.max(a.height, b.height) };
  if (b.y >= a.y + a.height) return { x: Math.min(a.x, b.x), y: a.y + a.height, width: Math.max(a.width, b.width), height: b.y - (a.y + a.height) };
  return undefined;
}

export function webWindowLayout(w: PlatformWindow): WindowLayout {
  const width = w.innerWidth ?? 0;
  const height = w.innerHeight ?? 0;
  const hinge = hingeOf(w.viewport?.segments);
  return { width, height, size: sizeClass(width), ...(hinge && { hinge }) };
}

/** A `wisp-back` event's detail as a BackEvent (the native side's predictive Back), or undefined. */
export function backEventOf(detail: unknown): BackEvent | undefined {
  if (typeof detail !== 'object' || detail === null) return undefined;
  const d = detail as Readonly<Record<string, unknown>>;
  const edge = d['edge'] === 'right' ? 'right' : 'left';
  switch (d['phase']) {
    case 'start':
      return { phase: 'start', edge };
    case 'progress': {
      const p = typeof d['progress'] === 'number' && Number.isFinite(d['progress']) ? d['progress'] : 0;
      return { phase: 'progress', edge, progress: Math.min(1, Math.max(0, p)) };
    }
    case 'cancel':
      return { phase: 'cancel' };
    case 'commit':
      return { phase: 'commit' };
    default:
      return undefined;
  }
}
