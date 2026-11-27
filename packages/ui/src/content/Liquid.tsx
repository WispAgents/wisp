// Liquid: the kit's one "busy" mark (owner 2026-11-27, ADR-0140). Three drops of the accent orbit one centre and run
// together like mercury: a blur and an alpha threshold (one shared SVG filter) melt the circles into one surface where
// they meet. Every loader in wisp draws it: a working turn, a button's loading, a pending act, a running step.
// Drawn on a 24 grid in currentColor, so it takes any size and tone; under reduced motion it holds one still frame.
import type { JSX } from 'solid-js';
import { flag, type KitProps } from '../core/props.ts';
import '../css/liquid.css';

export interface LiquidProps extends KitProps {
  /** waiting on you: the amber, slower */
  readonly waiting?: boolean;
  /** stopped (failed, idle): one still frame */
  readonly still?: boolean;
  /** what it's doing, for a screen reader; without it the mark is decoration */
  readonly label?: string;
}

const DEFS = 'k-liquid-defs';

/** The metaball filter, once per document (as the icon sprite): blur the drops, then cut their alpha sharp, so where
 *  two blurs overlap they read as one drop. Its numbers are in the 24 grid's units, so it scales with the mark. */
function ensureFilter(): void {
  if (typeof document === 'undefined' || document.getElementById(DEFS)?.isConnected) return;
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.id = DEFS;
  svg.setAttribute('aria-hidden', 'true');
  svg.setAttribute('style', 'position:absolute;width:0;height:0;overflow:hidden');
  // the kit's own constant markup, never user data
  svg.innerHTML =
    '<filter id="k-liquid" x="-50%" y="-50%" width="200%" height="200%" color-interpolation-filters="sRGB">' +
    '<feGaussianBlur stdDeviation="1.7"/><feColorMatrix values="1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 22 -9"/></filter>';
  document.body.prepend(svg);
}

export function Liquid(props: LiquidProps): JSX.Element {
  ensureFilter();
  return (
    <svg
      id={props.id}
      viewBox="0 0 24 24"
      data-kit="liquid"
      data-part={props.part ?? 'ui.liquid'}
      data-waiting={flag(props.waiting)}
      data-still={flag(props.still)}
      role={props.label ? 'img' : undefined}
      aria-label={props.label}
      aria-hidden={props.label ? undefined : 'true'}
    >
      <g filter="url(#k-liquid)">
        <circle data-kit="liquid-drop" cx="12" cy="12" r="4.1" />
        <circle data-kit="liquid-drop" cx="12" cy="12" r="3.3" />
        <circle data-kit="liquid-drop" cx="12" cy="12" r="3.3" />
      </g>
    </svg>
  );
}
