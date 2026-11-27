// PrPeek (ADR-0145): a PR named in a task's text as its live chip, in the sentence where it was written. The chip
// is the PR pill's (ToneChip: its tone's dot, "wisp #1056"); hovering or focusing it shows a card with what the PR
// is now (its title, state, head → base, its checks and review), so the reader never leaves the line to know if
// it's green. A click opens the PR pane. On a phone there is no hover: the tap opens the pane.
import { Show, type JSX } from 'solid-js';
import type { Tone } from '@wisp/tokens';
import { flag, sub, type KitProps, type PartName } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import { Tooltip } from '../controls/Tooltip.tsx';
import { ToneChip } from './ToneChip.tsx';
import '../css/ship.css';

export interface PrPeekProps extends KitProps {
  /** "wisp #1056" */
  readonly label: string;
  readonly tone: Tone;
  /** checks still running: the dot pulses */
  readonly live?: boolean;
  /** the PR's title */
  readonly title: string;
  /** "Open · 3 checks passed · Approved" */
  readonly line: string;
  readonly head?: string;
  readonly base?: string;
  readonly checks?: { readonly pass: number; readonly fail: number; readonly pending: number } | undefined;
  /** the card's last, quiet line ("Click to open the PR") */
  readonly hint?: string;
  readonly onOpen: () => void;
}

/** The counts row only says something the state line doesn't when the checks are mixed ("3 passed" is in the line). */
const mixed = (c: PrPeekProps['checks']): boolean => !!c && [c.pass, c.fail, c.pending].filter((n) => n > 0).length > 1;

export function PrPeek(props: PrPeekProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.pr-peek';
  const card = (): JSX.Element => (
    <div data-kit="pr-peek-card" data-part={sub(part(), 'card')}>
      <span data-kit="pr-peek-title">{props.title}</span>
      <span data-kit="pr-peek-line" data-tone={props.tone}>
        <span data-kit="tone-chip-dot" data-live={flag(props.live)} />
        {props.line}
      </span>
      <Show when={props.head && props.base}>
        <span data-kit="pr-peek-branches">
          <span data-kit="pr-peek-branch">{props.head}</span>
          <Icon name="arrow" size="sm" />
          <span data-kit="pr-peek-branch">{props.base}</span>
        </span>
      </Show>
      <Show when={mixed(props.checks) && props.checks}>
        {(c) => (
          <span data-kit="pr-peek-checks">
            <Show when={c().pass > 0}><span data-state="pass"><Icon name="check" size="xs" />{c().pass}</span></Show>
            <Show when={c().fail > 0}><span data-state="fail"><Icon name="x" size="xs" />{c().fail}</span></Show>
            <Show when={c().pending > 0}><span data-state="pending"><Icon name="clock" size="xs" />{c().pending}</span></Show>
          </span>
        )}
      </Show>
      <Show when={props.hint}>{(h) => <span data-kit="pr-peek-hint">{h()}</span>}</Show>
    </div>
  );
  return (
    <span id={props.id} data-kit="pr-peek" data-part={part()}>
      <Tooltip content={props.title} card={card} placement="top" openDelay={250} part={sub(part(), 'tip')}>
        <ToneChip label={props.label} tone={props.tone} {...(props.live !== undefined && { live: props.live })} onClick={props.onOpen} part={sub(part(), 'chip')} />
      </Tooltip>
    </span>
  );
}
