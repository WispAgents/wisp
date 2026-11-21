// Where a delegate runs, on its row in the lead's team panel (v1 team.js ranOn, subagents.css .tl-place): a small
// outlined chip, a laptop (this computer) or a server (a machine) and its name; on another machine than the lead
// (`away`) the name is in ink-2 and the icon in accent. A narrow row gives way to the time (v1 #799): the chip is
// capped and its name ellipsises, so the delegate's time stays inside the card. Its sheet loads with it (the Team
// tab is a lazy pane), so it adds nothing to the initial CSS.
import type { JSX } from 'solid-js';
import { flag, type KitProps } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import { fmtDuration } from '../blocks/content-util.ts';
import '../css/crew-team.css';

export interface TeamPlaceProps extends KitProps {
  /** the machine's name, or "This computer" */
  readonly name: string;
  /** laptop: this computer; server: a machine (v1: a machine's chip, or a lead that runs on one) */
  readonly icon: 'laptop' | 'server';
  /** on another machine than its lead */
  readonly away?: boolean;
  /** v1's tooltip: "Ran on Studio, in Ada's own folder" */
  readonly title?: string;
}

export function TeamPlace(props: TeamPlaceProps): JSX.Element {
  return (
    <span id={props.id} data-kit="team-place" data-part={props.part ?? 'ui.team-place'} data-away={flag(props.away)} title={props.title}>
      <Icon name={props.icon} size="xs" />
      <span>{props.name}</span>
    </span>
  );
}

export interface TeamTimeProps extends KitProps {
  /** how long it has run (a running one: now − its start; a finished one: end − start), in ms */
  readonly ms: number;
}

/** v1 .tl-row .elapsed / .sub-dur: "47s", "2m 14s", "1h 3m". Only its text reads `ms`, so a ticking clock repaints
 *  this one text node, never the row or the list. */
export function TeamTime(props: TeamTimeProps): JSX.Element {
  return (
    <span id={props.id} data-kit="team-time" data-text="control" data-part={props.part ?? 'ui.team-time'}>
      {fmtDuration(props.ms)}
    </span>
  );
}
