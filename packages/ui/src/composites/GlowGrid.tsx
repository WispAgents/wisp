// GlowGrid: a field of small square cells a wave of light runs across while something is being made far away (a
// sandbox on the grid), with what's happening in its middle. `working`: the accent wave, corner to corner; `waiting`: a
// slow breath, it waits for you; `failed`: still and dim, a few cells in the bad tone. The cells are decoration
// (aria-hidden); the middle says it.
import { For, Show, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import '../css/glow-grid.css';

export type GlowGridState = 'working' | 'waiting' | 'failed';

export interface GlowGridProps extends KitProps {
  readonly state: GlowGridState;
  /** what sits in the middle: an icon, a title, a line, a button */
  readonly children?: JSX.Element;
}

const COLS = 36;
const ROWS = 9;
// which cells read as broken when it failed: a few, scattered, the same every time
const BROKEN = new Set([40, 77, 118, 151, 200, 236, 263, 290, 311]);
const CELLS = Array.from({ length: COLS * ROWS }, (_, i) => i);

export function GlowGrid(props: GlowGridProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.glow-grid';
  return (
    <div id={props.id} data-kit="glow-grid" data-part={part()} data-state={props.state}>
      <div data-kit="glow-grid-cells" aria-hidden="true" style={{ '--k-cols': String(COLS) }}>
        <For each={CELLS}>
          {(i) => (
            <span
              data-kit="glow-cell"
              data-broken={props.state === 'failed' && BROKEN.has(i) ? '' : undefined}
              // the wave runs corner to corner: a cell's delay is its distance along the diagonal
              style={{ '--k-d': String((i % COLS) + Math.floor(i / COLS)) }}
            />
          )}
        </For>
      </div>
      <Show when={props.children}>
        <div data-kit="glow-grid-middle" data-part={sub(part(), 'middle')}>
          {props.children}
        </div>
      </Show>
    </div>
  );
}
