// BuildBox: a box that builds itself while something is being made far away (a sandbox on the grid), with what's
// happening under it. `working`: the edges draw in, the faces fill, the lid lifts and settles, and it starts over;
// `waiting`: the finished box breathes in the warn tone, it waits for you; `failed`: grey, the lid knocked off, a crack
// in the bad tone. The box is decoration (aria-hidden); the middle says it. Replaces the GlowGrid wave (owner, 2026-11-27).
import { Show, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import '../css/build-box.css';

export type BuildBoxState = 'working' | 'waiting' | 'failed';

export interface BuildBoxProps extends KitProps {
  readonly state: BuildBoxState;
  /** what sits under the box: an icon, a title, a line, a button */
  readonly children?: JSX.Element;
}

// an isometric box in a 120 × 120 view: left face, right face, the lid on top; each edge path draws in on its own delay
const LEFT = 'M28 52l32 16v34L28 86z';
const RIGHT = 'M92 52L60 68v34l32-16z';
const LID = 'M60 36l32 16-32 16-32-16z';

export function BuildBox(props: BuildBoxProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.build-box';
  return (
    <div id={props.id} data-kit="build-box" data-part={part()} data-state={props.state}>
      <svg data-kit="build-box-art" viewBox="0 0 120 120" aria-hidden="true">
        <ellipse data-kit="build-box-shadow" cx="60" cy="108" rx="34" ry="5" />
        <path data-kit="build-box-face" d={LEFT} style={{ '--k-o': '0.1' }} />
        <path data-kit="build-box-face" d={RIGHT} style={{ '--k-o': '0.22' }} />
        <path data-kit="build-box-edge" d="M28 52v34l32 16 32-16V52" />
        <path data-kit="build-box-edge" d="M60 68v34" style={{ '--k-d': '0.25s' }} />
        <g data-kit="build-box-lid">
          <path data-kit="build-box-face" d={LID} style={{ '--k-o': '0.35' }} />
          <path data-kit="build-box-edge" d={LID} style={{ '--k-d': '0.5s' }} />
        </g>
        <path data-kit="build-box-crack" d="M76 62l-4 9 5 6-4 10" />
      </svg>
      <Show when={props.children}>
        <div data-kit="build-box-middle" data-part={sub(part(), 'middle')}>
          {props.children}
        </div>
      </Show>
    </div>
  );
}
