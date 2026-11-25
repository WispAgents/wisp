// Section (a titled group, with actions on the right of its title) and SettingsRow (a short
// title, one line saying what it does, and its control in a fixed right column; copy.md).
// In the phone apps (ADR-0065 build step 4): Android's header is an M3 list subheader in the primary colour
// with no card; iOS draws an inset grouped section, and its description moves under the box as the
// section's footer (HIG: help text in footers).
import { createContext, createUniqueId, onMount, Show, useContext, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName, type WithChildren } from '../core/props.ts';
import type { HeadingLevel } from '../content/Text.tsx';
import { Heading } from '../content/Text.tsx';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { usePlatform } from '../platform/PlatformLook.tsx';
import '../css/settings.css';

export interface SectionProps extends KitProps, WithChildren {
  readonly title: string;
  readonly description?: JSX.Element;
  /** Buttons at the end of the title row. */
  readonly actions?: JSX.Element;
  readonly level?: HeadingLevel;
}

export function Section(props: SectionProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.section';
  const headingId = createUniqueId();
  const look = usePlatform();
  const footer = (): boolean => look() === 'ios';
  return (
    <section id={props.id} aria-labelledby={headingId} data-kit="section" data-part={part()}>
      <div data-kit="section-head" data-part={sub(part(), 'head')}>
        <div data-kit="section-titles">
          <Heading level={props.level ?? 3} part={sub(part(), 'title')} id={headingId}>
            {props.title}
          </Heading>
          <Show when={!footer() && props.description}>
            <p data-kit="section-description">{props.description}</p>
          </Show>
        </div>
        <Show when={props.actions}>
          <div data-kit="section-actions" data-part={sub(part(), 'actions')}>
            {props.actions}
          </div>
        </Show>
      </div>
      <div data-kit="section-body" data-part={sub(part(), 'body')}>
        {props.children}
      </div>
      <Show when={footer() && props.description}>
        <p data-kit="section-footer" data-part={sub(part(), 'footer')}>
          {props.description}
        </p>
      </Show>
    </section>
  );
}

export interface SettingsRowProps extends KitProps {
  readonly title: string;
  /** One line in the person's terms. */
  readonly description?: JSX.Element;
  /** A Switch, Select, Button…; none: the text takes the whole row (v1 .sx-ctl:empty) */
  readonly control?: JSX.Element;
  /** The control's id, so the title is its label. */
  readonly controlId?: string;
  /** A wide control (a Select, an Input) drops under the text on phones instead of squeezing it. */
  readonly wideControl?: boolean;
  /** a mark before the title: an image URL (a harness's logo, v1 Settings → Agents) */
  readonly logo?: string;
  /** a mark before the title from the icon set, where there is no logo (v1 GitHub, git) */
  readonly icon?: IconName;
  /** a 26px tile before the text, with an icon in it (v1 .cn-ico: Settings → Connectors) */
  readonly tile?: IconName;
  /** the title as the box's headline, 15px (v1 .wtr-total) */
  readonly big?: boolean;
  /** the control is a set of choices that wraps (v1 .ro-allowed: Auto's harnesses): at most 360px beside the text */
  readonly choices?: boolean;
  /** when the control drops under the text (a narrow pane), it keeps to the right (v1 .cn-found: Not now · Review) */
  readonly keepRight?: boolean;
  /** more under the words and the control, from the words' edge to the row's end, a hairline above (v1 .cg-agents: a
   *  cloud computer's agents). Additive
   *  (2026-11-17). */
  readonly below?: JSX.Element;
  /** the setting this row shows (`runtime.leanRuns`): Settings' search lands on it, scrolled to, flashed and focused
   *  when it's the one asked for (`SettingsFocus`). Additive (2026-11-25). */
  readonly settingKey?: string | undefined;
}

/** The setting Settings' search opened the pane on: its row scrolls into view, flashes once and takes focus. */
const FocusContext = createContext<() => string | undefined>(() => undefined);

export function SettingsFocus(props: { readonly setting: string | undefined; readonly children: JSX.Element }): JSX.Element {
  return <FocusContext.Provider value={() => props.setting}>{props.children}</FocusContext.Provider>;
}

const reduced = (): boolean => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;

/** The row asked for: in view (centred, at once with reduced motion), flashed, its control focused (else the row). */
function land(el: HTMLElement): void {
  el.scrollIntoView?.({ block: 'center', behavior: reduced() ? 'auto' : 'smooth' });
  el.setAttribute('data-found', '');
  el.addEventListener('animationend', () => el.removeAttribute('data-found'), { once: true });
  const ctl = el.querySelector<HTMLElement>('[data-kit="settings-row-control"] :is(button, input, select, textarea, [tabindex]):not([disabled])');
  (ctl ?? el).focus({ preventScroll: true });
}

// size-ok: one row's markup, its optional parts each a Show
export function SettingsRow(props: SettingsRowProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.settings-row';
  const focus = useContext(FocusContext);
  let box: HTMLDivElement | undefined;
  onMount(() => {
    const key = props.settingKey;
    // after the pane's first paint: the rows above it have their height, so the scroll lands where it stays
    if (key !== undefined && key === focus() && box) requestAnimationFrame(() => box && land(box));
  });
  return (
    <div
      ref={box}
      id={props.id}
      data-kit="settings-row"
      data-part={part()}
      data-key={props.settingKey}
      tabindex={props.settingKey !== undefined && props.settingKey === focus() ? -1 : undefined}
      data-wide={props.wideControl ? '' : undefined}
      data-layout={props.wideControl ? undefined : 'wrap'}
      data-tile={props.tile ? '' : undefined}
      data-bare={props.control === undefined ? '' : undefined}
      data-big={props.big ? '' : undefined}
      data-choices={props.choices ? '' : undefined}
      data-keep-right={props.keepRight ? '' : undefined}
    >
      <Show when={props.tile}>
        {(n) => (
          <span data-kit="settings-row-tile" aria-hidden="true">
            <Icon name={n()} size="sm" />
          </span>
        )}
      </Show>
      <div data-kit="settings-row-text">
        <Show
          when={props.controlId}
          fallback={
            <span data-kit="settings-row-title" data-text="control">
              <Show when={props.logo}>{(src) => <img data-kit="settings-row-logo" src={src()} alt="" />}</Show>
              <Show when={props.icon}>{(n) => <Icon name={n()} size="sm" />}</Show>
              {/* the words in a box of their own: the title is a flex row, and a flex container's own text is cut with
                  no "…" (QA r1 #21) */}
              <span data-kit="settings-row-words" title={props.title}>
                {props.title}
              </span>
            </span>
          }
        >
          {(forId) => (
            <label data-kit="settings-row-title" data-text="control" for={forId()}>
              <Show when={props.logo}>{(src) => <img data-kit="settings-row-logo" src={src()} alt="" />}</Show>
              <Show when={props.icon}>{(n) => <Icon name={n()} size="sm" />}</Show>
              {/* the words in a box of their own: the title is a flex row, and a flex container's own text is cut with
                  no "…" (QA r1 #21) */}
              <span data-kit="settings-row-words" title={props.title}>
                {props.title}
              </span>
            </label>
          )}
        </Show>
        <Show when={props.description}>
          <span data-kit="settings-row-description">{props.description}</span>
        </Show>
      </div>
      <Show when={props.control !== undefined}>
        <div data-kit="settings-row-control" data-part={sub(part(), 'control')}>
          {props.control}
        </div>
      </Show>
      <Show when={props.below}>
        <div data-kit="settings-row-below" data-part={sub(part(), 'below')}>
          {props.below}
        </div>
      </Show>
    </div>
  );
}
