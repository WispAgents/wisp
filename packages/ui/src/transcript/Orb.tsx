// Orb: live work. It draws the kit's Liquid (ADR-0140, which replaced v1's 3×3 ripple grid): the one "busy" signal
// (design/ui.md: don't invent another). Amber and slower while it waits on you; still when not live, and under reduced
// motion.
import type { JSX } from 'solid-js';
import { flag, type KitProps } from '../core/props.ts';
import { Liquid } from '../content/Liquid.tsx';
import '../css/transcript.css';

export interface OrbProps extends KitProps {
  readonly live?: boolean;
  /** waiting on you: the amber, slower */
  readonly waiting?: boolean;
  readonly label?: string;
}

export function Orb(props: OrbProps): JSX.Element {
  return (
    <span
      id={props.id}
      data-kit="orb"
      data-part={props.part ?? 'ui.orb'}
      data-live={flag(props.live)}
      data-waiting={flag(props.waiting)}
      role={props.label ? 'img' : undefined}
      aria-label={props.label}
      aria-hidden={props.label ? undefined : 'true'}
    >
      <Liquid waiting={props.waiting ?? false} still={!props.live} />
    </span>
  );
}
