// Facts: a short list of what something is, one label and its value a line (Machines' detail: Working, Address,
// Agents, Connects). The labels sit in one column; under 480px each value drops under its label.
import { For, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import '../css/facts.css';

export interface Fact {
  readonly label: string;
  readonly value: JSX.Element;
  /** its own data-part, where a plugin may restyle one line */
  readonly part?: PartName;
}

export interface FactsProps extends KitProps {
  readonly items: readonly Fact[];
}

export function Facts(props: FactsProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.facts';
  return (
    <dl id={props.id} data-kit="facts" data-part={part()}>
      <For each={props.items}>
        {(f) => (
          <div data-kit="fact" data-part={f.part ?? sub(part(), 'item')}>
            <dt>{f.label}</dt>
            <dd>{f.value}</dd>
          </div>
        )}
      </For>
    </dl>
  );
}
