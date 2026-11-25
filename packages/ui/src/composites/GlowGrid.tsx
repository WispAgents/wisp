// GlowGrid: a field of small cells a wave of light runs across while something is being made far away (a sandbox
// on the grid), with what's happening in its middle. `working`: the accent wave; `waiting`: a slow breath, it waits
// for you; `failed`: still, a few cells in the bad tone. The cells are decoration (aria-hidden); the middle says it.
import { For, Show, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import '../css/glow-grid.css';

export type GlowGridState = 'working' | 'waiting' | 'failed';

export interface GlowGridProps extends KitProps {
  readonly state: GlowGridState;
  /** what sits in the middle: an icon, a title, a line */
  readonly children?: JSX.Element;
  /** `sm`: a row's mark (4 × 3 cells); `lg` (default): a page's hero */
  readonly size?: 'sm' | 'lg';
}

const COLS = { sm: 4, lg: 16 } as const;
const ROWS = { sm: 3, lg: 7 } as const;
// which cells read as broken when it failed: a few, scattered, the same every time
const BROKEN = new Set([5, 19, 22, 40, 58, 61, 83, 97, 100]);

export function GlowGrid(props: GlowGridProps): JSX.Element {
  const size = (): 'sm' | 'lg' => props.size ?? 'lg';
  const part = (): PartName => props.part ?? 'ui.glow-grid';
  const cells = (): readonly number[] => Array.from({ length: COLS[size()] * ROWS[size()] }, (_, i) => i);
  return (
    <div id={props.id} data-kit="glow-grid" data-part={part()} data-state={props.state} data-size={size()}>
      <div data-kit="glow-grid-cells" aria-hidden="true" style={{ '--k-cols': String(COLS[size()]) }}>
        <For each={cells()}>
          {(i) => {
            const cols = COLS[size()];
            // the wave runs corner to corner: a cell's delay is its distance along the diagonal
            const d = (i % cols) + Math.floor(i / cols);
            return <span data-kit="glow-cell" data-broken={props.state === 'failed' && BROKEN.has(i) ? '' : undefined} style={{ '--k-d': String(d) }} />;
          }}
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
