// Reserved: holds the room its content took last time while that content loads (a plugin's row drawn after the first
// screen), so it lands without pushing what's under it (ADR-0017: nothing jumps). The caller keeps the height it
// reports (`onHeight`, on change, never 0 before the content first showed; 0 when it goes having never shown) and passes it back as `height`.
import { onCleanup, onMount, type JSX } from 'solid-js';
import type { KitProps, WithChildren } from '../core/props.ts';

export interface ReservedProps extends KitProps, WithChildren {
  /** the room to hold, in px (what `onHeight` said last time); 0 or unset: none */
  readonly height?: number | undefined;
  /** the content's own height as it changes: a positive one, then 0 only if it showed and went away */
  readonly onHeight?: ((px: number) => void) | undefined;
}

export function Reserved(props: ReservedProps): JSX.Element {
  let inner: HTMLDivElement | undefined;
  let shown = false;
  // held room whose content never showed while it was up (a plugin turned off, nothing to show any more): let it go,
  // so an empty gap isn't kept for good
  onCleanup(() => {
    if (!shown && props.height) props.onHeight?.(0);
  });
  onMount(() => {
    if (!inner || typeof ResizeObserver !== 'function') return;
    let last = -1;
    const ro = new ResizeObserver(() => {
      // measured from the content box, never the reserved one: holding the room changes nothing it reads (no loop)
      const h = Math.round(inner?.offsetHeight ?? 0);
      if (h > 0) shown = true;
      if ((h === 0 && !shown) || h === last) return;
      last = h;
      props.onHeight?.(h);
    });
    ro.observe(inner);
    onCleanup(() => ro.disconnect());
  });
  return (
    <div
      id={props.id}
      data-kit="reserved"
      data-part={props.part ?? 'ui.reserved'}
      style={props.height ? { '--reserve-h': `${props.height}px` } : undefined}
    >
      <div data-kit="reserved-body" ref={(el) => (inner = el)}>
        {props.children}
      </div>
    </div>
  );
}
