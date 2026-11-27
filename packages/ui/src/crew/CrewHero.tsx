// The top of a crew member's page (ADR-0140): the face in its colour's glow, the name, the role, the chips (what
// it's doing, how it runs) and the numbers its jobs back up. The page (CrewView) sets the colour (`data-color`).
import { For, Show, type JSX } from 'solid-js';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';

/** A chip under the name: what it's doing (`tone`), or how it runs (an icon or the harness's logo). */
export interface CrewChip {
  readonly text: string;
  readonly icon?: IconName;
  readonly logo?: string | undefined;
  readonly tone?: 'asks' | 'live' | 'report' | 'paused' | '';
  readonly title?: string;
  readonly onClick?: (() => void) | undefined;
}

export interface CrewHeroProps {
  readonly face: JSX.Element;
  readonly name: string;
  readonly role?: string | undefined;
  readonly chips: readonly CrewChip[];
  readonly stats: readonly { readonly value: string; readonly label: string }[];
}

const chip = (c: CrewChip): JSX.Element => {
  const inner = (): JSX.Element => (
    <>
      <Show
        when={c.tone === 'live' || c.tone === 'asks'}
        fallback={
          <Show when={c.logo} fallback={<Show when={c.icon}>{(i) => <Icon name={i()} />}</Show>}>
            {(src) => (
              <i data-kit="cv-logo">
                <img src={src()} alt="" />
              </i>
            )}
          </Show>
        }
      >
        <span data-kit="cv-pulse" aria-hidden="true" />
      </Show>
      <span data-text="control">{c.text}</span>
    </>
  );
  return (
    <Show
      when={c.onClick}
      fallback={
        <span data-kit="cv-chip" data-tone={c.tone || undefined} title={c.title ?? c.text}>
          {inner()}
        </span>
      }
    >
      {(go) => (
        <button type="button" data-kit="cv-chip" data-tone={c.tone || undefined} title={c.title ?? c.text} onClick={() => go()()}>
          {inner()}
        </button>
      )}
    </Show>
  );
};

export function CrewHero(props: CrewHeroProps): JSX.Element {
  return (
    <div data-kit="cv-hero" data-part="crew.hero">
      <span data-kit="cv-glow" aria-hidden="true" />
      <span data-kit="cv-halo">{props.face}</span>
      <h2 data-kit="cv-name" data-text="control">
        {props.name}
      </h2>
      <Show when={props.role}>
        <p data-kit="cv-role" data-text="control">
          {props.role}
        </p>
      </Show>
      <div data-kit="cv-chips" data-part="crew.chips" data-layout="wrap">
        <For each={props.chips}>{chip}</For>
      </div>
      <Show when={props.stats.length > 0}>
        <dl data-kit="cv-stats" data-part="crew.stats" data-layout="wrap">
          <For each={props.stats}>
            {(s) => (
              <div data-kit="cv-stat">
                <dt>{s.label}</dt>
                <dd data-text="control">{s.value}</dd>
              </div>
            )}
          </For>
        </dl>
      </Show>
    </div>
  );
}
