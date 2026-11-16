// A team's panel (v1 subagents.css .team-pop, judges.js judgesEditor): what the team is and does in its head, how
// they decide (Met when: a segmented All · Majority · Any with the rule in words), a row per member (or the empty
// note with its icon in a dashed box), and the foot with Add, "n of max" and Clear. It goes in a bare Popover. The
// goal's judges use it; the rows' contents are the caller's (a member's controls, a crew face and name).

import { For, Show, type JSX } from 'solid-js';
import { Icon } from '../content/Icon.tsx';
import { Button } from '../controls/Button.tsx';
import type { IconName } from '../content/icons.ts';
import { flag, type KitProps, type PartName } from '../core/props.ts';
import '../css/app-team.css';

export interface TeamVote {
  readonly label: string;
  readonly options: readonly { readonly value: string; readonly label: string; readonly hint: string }[];
  readonly value: string;
  /** nothing to decide yet (fewer than two): shown dimmed, a press does nothing */
  readonly off?: boolean;
  /** the rule in words, after the segments */
  readonly rule: string;
  readonly onChange: (value: string) => void;
}

export interface TeamPopProps extends KitProps {
  readonly title: string;
  readonly intro: string;
  readonly vote?: TeamVote;
  /** how many rows there are: none shows `empty` instead */
  readonly count: number;
  readonly empty: { readonly icon: IconName; readonly text: string };
  /** the rows (TeamRow each) */
  readonly children?: JSX.Element;
  /** Add (a button or a menu), at the foot's start */
  readonly add: JSX.Element;
  /** "0 of 5" */
  readonly tally: string;
  readonly clear?: { readonly label: string; readonly onClick: () => void } | undefined;
}

export function TeamPop(props: TeamPopProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.teamPop';
  return (
    <div data-kit="team-pop" data-part={part()} role="group" aria-label={props.title}>
      <header data-kit="tp-head">
        <div data-kit="tp-title">
          <b>{props.title}</b>
          <small>{props.intro}</small>
        </div>
      </header>
      <Show when={props.vote}>
        {(v) => (
          <div data-kit="tp-vote" data-off={flag(v().off)} data-part={`${part()}.vote`}>
            <span data-kit="tp-vote-k">{v().label}</span>
            <span data-kit="tp-seg" role="radiogroup" aria-label={v().label} aria-disabled={v().off === true}>
              <For each={v().options}>
                {(o) => (
                  <button type="button" role="radio" aria-checked={o.value === v().value} title={o.hint} onClick={() => !v().off && o.value !== v().value && v().onChange(o.value)}>
                    {o.label}
                  </button>
                )}
              </For>
            </span>
            <small data-kit="tp-vote-rule" data-part={`${part()}.rule`}>
              {v().rule}
            </small>
          </div>
        )}
      </Show>
      <div data-kit="tp-list">
        <Show
          when={props.count > 0}
          fallback={
            <div data-kit="tp-empty">
              <Icon name={props.empty.icon} />
              <p>{props.empty.text}</p>
            </div>
          }
        >
          {props.children}
        </Show>
      </div>
      <footer data-kit="tp-foot">
        {props.add}
        <span data-kit="tp-count">{props.tally}</span>
        <span data-kit="grow" />
        <Show when={props.clear}>
          {(c) => (
            <Button variant="link" size="xs" part={`${part()}.clear`} onClick={() => c().onClick()}>
              {c().label}
            </Button>
          )}
        </Show>
      </footer>
    </div>
  );
}

/** One member's row: its controls or face and name, then Remove (v1 .tp-row .tp-top … .tp-del). */
export function TeamRow(props: { readonly crew?: boolean; readonly part?: PartName; readonly remove: JSX.Element; readonly children?: JSX.Element }): JSX.Element {
  return (
    <div data-kit="tp-row" data-crew={flag(props.crew)} data-part={props.part ?? 'ui.teamRow'}>
      <div data-kit="tp-top">
        {props.children}
        <span data-kit="grow" />
        {props.remove}
      </div>
    </div>
  );
}

/** A crew member in a row: their face, name and what they are (v1 .tp-crew). */
export function TeamCrew(props: { readonly face: JSX.Element; readonly name: string; readonly sub: string }): JSX.Element {
  return (
    <>
      {props.face}
      <span data-kit="tp-crew">
        <b>{props.name}</b>
        <small>{props.sub}</small>
      </span>
    </>
  );
}
