// A team's panel (v1 subagents.css .team-pop, judges.js judgesEditor): what the team is and does in its head, how
// they decide (Met when: a segmented All · Majority · Any with the rule in words), a row per member (or the empty
// note with its icon in a dashed box), and the foot with Add, "n of max" and Clear. It goes in a bare Popover. The
// goal's judges use it; the rows' contents are the caller's (a member's controls, a crew face and name). Delegate (v1
// team.js teamEditor) uses it wide, with the lead's strip under the head and a second line per row (TeamLead,
// TeamWhere, TeamCopies, TeamFields).

import { For, Show, createSignal, type JSX } from 'solid-js';
import { Icon } from '../content/Icon.tsx';
import { Button } from '../controls/Button.tsx';
import { Input } from '../controls/Input.tsx';
import type { IconName } from '../content/icons.ts';
import { flag, type KitProps, type PartName } from '../core/props.ts';
import { WithAct, type Act } from './act.tsx';
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
  /** v1's Delegate panel: 560 wide (the judges' is 480). Additive (2026-11-17). */
  readonly wide?: boolean;
  /** Under the head: the lead's strip (TeamLead). Additive (2026-11-17). */
  readonly lead?: JSX.Element;
  /** At the head's end: Delegate's Presets (v1 .tp-presets). Additive (2026-11-17). */
  readonly head?: JSX.Element;
  /** At the foot's end, after Clear: Save as preset (v1 .tp-save). Additive (2026-11-17). */
  readonly end?: JSX.Element;
  /** In place of the foot while it's there: Save as preset's name (TeamSave, v1 .tp-saveform). Additive (2026-11-17). */
  readonly form?: JSX.Element;
}

export function TeamPop(props: TeamPopProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.teamPop';
  return (
    <div data-kit="team-pop" data-part={part()} data-wide={flag(props.wide)} role="group" aria-label={props.title}>
      <header data-kit="tp-head">
        <div data-kit="tp-title">
          <b>{props.title}</b>
          <small>{props.intro}</small>
        </div>
        {props.head}
      </header>
      {props.lead}
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
        <Show
          when={props.form}
          fallback={
            <>
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
              {props.end}
            </>
          }
        >
          {props.form}
        </Show>
      </footer>
    </div>
  );
}

/** One member's row: its controls or face and name, then Remove (v1 .tp-row .tp-top … .tp-del); a delegate's second
 *  line (its @name and what it's for, v1 .tp-bottom) under them. */
export function TeamRow(props: { readonly crew?: boolean; readonly part?: PartName; readonly remove: JSX.Element; readonly bottom?: JSX.Element; readonly children?: JSX.Element }): JSX.Element {
  return (
    <div data-kit="tp-row" data-crew={flag(props.crew)} data-part={props.part ?? 'ui.teamRow'}>
      <div data-kit="tp-top">
        {props.children}
        <span data-kit="grow" />
        <span data-kit="tp-end">{props.remove}</span>
      </div>
      <Show when={props.bottom}>
        <div data-kit="tp-bottom">{props.bottom}</div>
      </Show>
    </div>
  );
}

/** Where a place is: with the lead (the team's icon), this computer, a machine or a sandbox. */
export interface TeamPlaceLook {
  readonly icon: IconName;
  readonly label: string;
  readonly title: string;
  /** away from the lead: the icon in the accent (v1 .tp-place.away) */
  readonly away?: boolean;
  /** a machine that isn't connected: its name dimmed (v1 .tp-place.off) */
  readonly off?: boolean;
}

/** Where a delegate (or the lead) runs (v1 .tp-place): a chip that opens its menu, or a dashed static one. */
export function TeamWhere(props: { readonly place: TeamPlaceLook; readonly act?: Act | undefined; readonly part?: PartName }): JSX.Element {
  const inner = (): JSX.Element => (
    <>
      <Icon name={props.place.icon} />
      <span>{props.place.label}</span>
    </>
  );
  return (
    <Show
      when={props.act}
      fallback={
        <span data-kit="tp-place" data-static="" data-part={props.part ?? 'ui.teamPlace'} title={props.place.title}>
          {inner()}
        </span>
      }
    >
      <WithAct
        act={props.act}
        render={(trigger) => (
          <button type="button" {...trigger()} data-kit="tp-place" data-away={flag(props.place.away)} data-off={flag(props.place.off)} data-part={props.part ?? 'ui.teamPlace'} title={props.place.title}>
            {inner()}
            <Icon name="chevron" size="xs" />
          </button>
        )}
      />
    </Show>
  );
}

/** The lead's strip (v1 .tp-lead): LEAD, its logo, harness · model, "on" and where it runs. */
export function TeamLead(props: {
  readonly tag: string;
  readonly logo?: string | undefined;
  readonly run: string;
  readonly on: string;
  readonly place: TeamPlaceLook;
  readonly placeAct?: Act | undefined;
  readonly part?: PartName;
}): JSX.Element {
  return (
    <div data-kit="tp-lead" data-part={props.part ?? 'ui.teamLead'}>
      <span data-kit="tp-tag">{props.tag}</span>
      <span data-kit="tp-lead-logo">
        <Show when={props.logo} fallback={<Icon name="spark" size="sm" />}>
          {(src) => <img src={src()} alt="" />}
        </Show>
      </span>
      <span data-kit="tp-lead-run" title={props.run}>
        {props.run}
      </span>
      <span data-kit="tp-on">{props.on}</span>
      <TeamWhere place={props.place} act={props.placeAct} part={`${props.part ?? 'ui.teamLead'}.place`} />
    </div>
  );
}

/** Copies of one delegate (v1 .tp-copies): − ×n +, accent once there's more than one. */
export function TeamCopies(props: {
  readonly n: number;
  readonly max: number;
  readonly title: string;
  readonly fewer: string;
  readonly more: string;
  readonly onChange: (n: number) => void;
  readonly part?: PartName;
}): JSX.Element {
  return (
    <span data-kit="tp-copies" data-on={flag(props.n > 1)} data-part={props.part ?? 'ui.teamCopies'} title={props.title}>
      <button type="button" aria-label={props.fewer} disabled={props.n <= 1} onClick={() => props.n > 1 && props.onChange(props.n - 1)}>
        <Icon name="minus" size="xs" />
      </button>
      <b>×{props.n}</b>
      <button type="button" aria-label={props.more} disabled={props.n >= props.max} onClick={() => props.n < props.max && props.onChange(props.n + 1)}>
        <Icon name="plus" size="xs" />
      </button>
    </span>
  );
}

/** A delegate's second line (v1 .tp-name, .tp-use): what the lead calls it (@name) and what it's for. */
export function TeamFields(props: {
  readonly name: { readonly value: string; readonly placeholder: string; readonly title: string; readonly onInput: (v: string) => void };
  readonly use: { readonly label: string; readonly value: string; readonly placeholder: string; readonly onInput: (v: string) => void };
  readonly part?: PartName;
}): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.teamFields';
  return (
    <>
      <span data-kit="tp-name" title={props.name.title}>
        <Input size="sm" mono prefix="@" value={props.name.value} placeholder={props.name.placeholder} maxLength={24} spellcheck={false} autocomplete="off" aria-label={props.name.title} onInput={props.name.onInput} part={`${part()}.name`} />
      </span>
      <span data-kit="tp-use">
        <Input size="sm" prefix={props.use.label} value={props.use.value} placeholder={props.use.placeholder} maxLength={300} spellcheck={false} autocomplete="off" aria-label={props.use.label} onInput={props.use.onInput} part={`${part()}.use`} />
      </span>
    </>
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

/** Save as preset's name (v1 team.js askName, .tp-saveform): a field, Save, and Cancel. Enter saves; an empty name
 *  keeps the field focused and saves nothing. */
export function TeamSave(props: {
  readonly value: string;
  readonly placeholder: string;
  readonly save: string;
  readonly cancel: string;
  readonly onSave: (name: string) => void;
  readonly onCancel: () => void;
  readonly part?: PartName;
}): JSX.Element {
  let input: HTMLInputElement | undefined;
  const [name, setName] = createSignal(props.value);
  const submit = (e: SubmitEvent): void => {
    e.preventDefault();
    const n = name().trim();
    if (!n) return void input?.focus();
    props.onSave(n);
  };
  return (
    <form data-kit="tp-saveform" data-part={props.part ?? 'ui.teamSave'} onSubmit={submit}>
      <Input
        size="sm"
        value={name()}
        placeholder={props.placeholder}
        maxLength={60}
        aria-label={props.placeholder}
        autofocus
        ref={(el) => {
          input = el;
          queueMicrotask(() => (el.focus(), el.select()));
        }}
        onInput={setName}
        onKeyDown={(e) => e.key === 'Escape' && (e.stopPropagation(), props.onCancel())}
        part={`${props.part ?? 'ui.teamSave'}.name`}
      />
      <Button type="submit" variant="primary" size="sm" part={`${props.part ?? 'ui.teamSave'}.save`}>
        {props.save}
      </Button>
      <Button variant="link" size="xs" part={`${props.part ?? 'ui.teamSave'}.cancel`} onClick={() => props.onCancel()}>
        {props.cancel}
      </Button>
    </form>
  );
}
