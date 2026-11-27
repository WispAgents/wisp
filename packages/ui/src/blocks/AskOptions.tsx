// A question's answers (v1 .q-opts .opt): numbered, bordered, left-aligned choices that wrap onto more lines rather
// than cut their text, so a long answer reads whole. The `question` block and the task's live ask card share it.
import { createMemo, For, Show, type JSX } from 'solid-js';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { Markdown } from '../content/markdown/Markdown.tsx';
import { Wrap } from '../layout/Stack.tsx';
import { sub, type PartName } from '../core/props.ts';
import { splitQuestion } from './ask-split.ts';
import '../css/answer-rich.css';

export interface AskOption {
  readonly label: string;
  readonly detail?: string | undefined;
}

export function AskOptions(props: {
  readonly options: readonly AskOption[];
  readonly onPick: (index: number) => void;
  readonly disabled?: boolean;
  readonly part?: PartName;
  readonly optionPart?: PartName;
}): JSX.Element {
  return (
    <Wrap gap="4" part={props.part ?? 'ui.ask.options'}>
      <For each={props.options}>
        {(o, i) => (
          <button type="button" data-kit="rb-opt" data-part={props.optionPart ?? 'ui.ask.option'} disabled={props.disabled} onClick={() => props.onPick(i())}>
            <span data-kit="rb-opt-k">{i() + 1}</span>
            <span>
              {o.label}
              <Show when={o.detail}>{(d) => <small>{d()}</small>}</Show>
            </span>
          </button>
        )}
      </For>
    </Wrap>
  );
}

/** v1 .ask-card: what the agent waits on. The head (icon and label, warn-coloured), the question's first paragraph as
 *  its headline, the rest as smaller muted context (v1 .q-text / .q-context), then the answers and a foot note.
 *  Agents put their whole explanation in one question; splitting it keeps the call itself readable. */
export function AskPanel(props: {
  readonly label: string;
  readonly question: string;
  readonly icon?: IconName;
  readonly part?: PartName;
  readonly 'aria-label'?: string;
  readonly children?: JSX.Element;
  readonly foot?: JSX.Element;
}): JSX.Element {
  const split = createMemo(() => splitQuestion(props.question));
  return (
    <section data-kit="rb-question" data-tone="warn" data-part={props.part ?? 'ui.ask'} aria-label={props['aria-label']}>
      <div data-kit="rb-q-head">
        <Icon name={props.icon ?? 'ask'} size="sm" />
        <span>{props.label}</span>
      </div>
      <div data-kit="rb-q-text" data-more={split().context ? '' : undefined}>
        <Markdown source={split().headline} part={sub(props.part ?? 'ui.ask', 'question')} />
      </div>
      <Show when={split().context}>
        {(c) => (
          <div data-kit="rb-q-context">
            <Markdown source={c()} part={sub(props.part ?? 'ui.ask', 'context')} />
          </div>
        )}
      </Show>
      {props.children}
      <Show when={props.foot}>
        <div data-kit="rb-q-foot">{props.foot}</div>
      </Show>
    </section>
  );
}
