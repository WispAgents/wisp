// size-ok: the page and the parts drawn inside it (job cards, facts, sections) are one kit area, read together
// A crew member's page (design/crew.md "The member's page", ADR-0140): a profile, not a log. A hero at the top
// (the face in its colour's glow, the name, the role, chips for what it's doing and how it runs, a line of
// numbers), tabs under it (Activity is the thread of jobs; the plugin adds Kit, Notebook, How they work), and the
// box that gives it a job, with the jobs you gave it before as one-click starts. Data and words come from the
// crew plugin.
import { createMemo, createSignal, For, Match, Show, Switch, type JSX } from 'solid-js';
import { flag } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { Tabs } from '../controls/Tabs.tsx';
import { CrewHero, type CrewChip } from './CrewHero.tsx';
import { WithAct, type Act } from '../app/act.tsx';
import '../css/crew-view.css';

export type { CrewChip } from './CrewHero.tsx';

export interface CrewTab {
  readonly value: string;
  readonly label: string;
  readonly count?: number | undefined;
  /** the panel's data-part, so a plugin can restyle it (crew.kit, crew.notebook, crew.details) */
  readonly part: string;
  /** drawn only while its tab is open */
  readonly content: () => JSX.Element;
}

export interface CrewViewProps {
  /** the hero's face (Face size="hero") */
  readonly face: JSX.Element;
  /** the member's colour: the hero's glow */
  readonly color: string;
  readonly name: string;
  readonly role?: string | undefined;
  readonly chips: readonly CrewChip[];
  /** a few numbers from its jobs ("12 jobs done"); none hides the line */
  readonly stats: readonly { readonly value: string; readonly label: string }[];
  /** the head's buttons (Pause, More, Close), the plugin's kit controls */
  readonly actions: JSX.Element;
  /** stack: back to the list */
  readonly back?: { readonly label: string; readonly onClick: () => void } | undefined;
  /** the Activity tab's label, and its jobs (CrewJob, CrewQuiet, CrewEmpty) */
  readonly activity: string;
  readonly thread: JSX.Element;
  readonly earlier?: { readonly label: string; readonly onClick: () => void } | undefined;
  /** the tabs after Activity */
  readonly tabs: readonly CrewTab[];
  readonly tabsLabel: string;
  /** above the box: the jobs you gave it before, one click puts one in the box */
  readonly suggestions?: readonly {
    readonly label: string;
    readonly onClick: () => void;
  }[];
  readonly dock: {
    readonly placeholder: string;
    readonly value: string;
    readonly onInput: (v: string) => void;
    readonly onSend: () => void;
    readonly send: string;
    readonly checkIn: {
      readonly label: string;
      readonly title: string;
      readonly act: Act;
    };
    readonly inputRef?: (el: HTMLTextAreaElement) => void;
  };
}

/** The page: the head's buttons over the hero, the tabs, the box pinned at the foot. */
export function CrewView(props: CrewViewProps): JSX.Element {
  const [tab, setTab] = createSignal('activity');
  // the tab row is labels only: a panel is drawn when its tab opens, never while the row is read
  const items = createMemo(() => [
    { value: 'activity', label: props.activity },
    ...props.tabs.map((t) => ({
      value: t.value,
      label: t.label,
      ...(t.count ? { count: t.count } : {}),
    })),
  ]);
  const open = createMemo(() => props.tabs.find((t) => t.value === tab()));
  const key = (e: KeyboardEvent): void => {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      props.dock.onSend();
    }
  };
  return (
    <section data-kit="crew-view" data-part="crew.page" data-color={props.color}>
      <header data-kit="cv-top" data-part="crew.page.head">
        <Show when={props.back}>
          {(b) => (
            <button type="button" data-kit="head-btn" data-part="crew.page.back" title={b().label} aria-label={b().label} onClick={() => b().onClick()}>
              <Icon name="back" />
            </button>
          )}
        </Show>
        <span data-kit="grow" />
        <span data-kit="sh-acts">{props.actions}</span>
      </header>
      <div data-kit="cv-scroll">
        <CrewHero face={props.face} name={props.name} role={props.role} chips={props.chips} stats={props.stats} />
        <div data-kit="cv-tabs" data-part="crew.tabs">
          <Tabs aria-label={props.tabsLabel} value={tab()} onChange={setTab} items={items()} />
          <div data-kit="cv-tab-body" role="tabpanel" aria-label={items().find((i) => i.value === tab())?.label}>
            <Switch>
              <Match when={tab() === 'activity' || !open()}>
                <div data-kit="cv-main">
                  <Show when={props.earlier}>
                    {(e) => (
                      <button type="button" data-kit="link" data-part="crew.page.earlier" data-cv="earlier" onClick={() => e().onClick()}>
                        {e().label}
                      </button>
                    )}
                  </Show>
                  <div data-kit="cv-thread" data-part="crew.thread">
                    {props.thread}
                  </div>
                </div>
              </Match>
              <Match when={open()} keyed>
                {(t) => (
                  <div data-kit="cv-panel" data-part={t.part}>
                    {t.content()}
                  </div>
                )}
              </Match>
            </Switch>
          </div>
        </div>
        <div data-kit="cv-dock" data-part="crew.dock crew.compose">
          <Show when={tab() === 'activity' && (props.suggestions?.length ?? 0) > 0}>
            <div data-kit="cv-suggest" data-part="crew.suggest">
              <For each={props.suggestions}>
                {(s) => (
                  <button type="button" data-kit="cv-chip" data-go="" title={s.label} onClick={() => s.onClick()}>
                    <Icon name="replay" />
                    <span data-text="control">{s.label}</span>
                  </button>
                )}
              </For>
            </div>
          </Show>
          <div data-kit="cv-box">
            <textarea
              data-kit="cv-input"
              data-part="crew.dock.input"
              rows="1"
              spellcheck={false}
              placeholder={props.dock.placeholder}
              value={props.dock.value}
              ref={(el) => props.dock.inputRef?.(el)}
              onInput={(e) => props.dock.onInput(e.currentTarget.value)}
              onKeyDown={key}
            />
            <div data-kit="cv-bar">
              <WithAct
                act={props.dock.checkIn.act}
                render={(trigger) => (
                  <button type="button" {...trigger()} data-kit="cv-ghost" data-part="crew.dock.checkin" title={props.dock.checkIn.title}>
                    <Icon name="clock" />
                    <span data-text="control">{props.dock.checkIn.label}</span>
                  </button>
                )}
              />
              <span data-kit="grow" />
              <button type="button" data-kit="send" data-part="crew.dock.send" title={props.dock.send} aria-label={props.dock.send} onClick={() => props.dock.onSend()}>
                <Icon name="enter" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

export interface CrewJobProps {
  readonly id: string;
  /** where it came from: v1 .cv-kind ("You", "Check-in", "Tagged in …") */
  readonly kind: { readonly icon: IconName; readonly label: string; readonly you?: boolean };
  readonly time: string;
  readonly timeTitle?: string;
  /** what it was asked */
  readonly prompt?: string | undefined;
  readonly face: JSX.Element;
  readonly pill?: { readonly label: string; readonly tone: string } | undefined;
  /** working: its live line; else its title and summary */
  readonly live?: string | undefined;
  readonly title?: string;
  readonly summary?: string | undefined;
  /** asking you: the question */
  readonly question?: string | undefined;
  readonly unread?: boolean;
  readonly state: string;
  /** Open job, Follow up, Save for later */
  readonly actions: readonly { readonly icon: IconName; readonly label: string; readonly on?: boolean; readonly onClick: () => void }[];
  /** under its actions: what the job proposed for the kit (v1 .cv-offer) */
  readonly extra?: JSX.Element;
}

/** v1 .cv-job: the request (where from, when, what was asked), then the member's answer. */
export function CrewJob(props: CrewJobProps): JSX.Element {
  return (
    <article data-kit="cv-job" data-part="crew.job" data-state={props.state} data-unread={flag(props.unread)}>
      <div data-kit="cv-req">
        <span data-kit="cv-kind" data-you={flag(props.kind.you)}>
          <Icon name={props.kind.icon} />
          <span data-text="control">{props.kind.label}</span>
        </span>
        <time data-kit="cv-time" title={props.timeTitle}>
          {props.time}
        </time>
        <Show when={props.prompt}>
          <div data-kit="cv-prompt">{props.prompt}</div>
        </Show>
      </div>
      <div data-kit="cv-res">
        {props.face}
        <div data-kit="cv-say">
          <Show when={props.pill}>
            {(p) => (
              <div data-kit="cv-head">
                <span data-kit="it-pill" data-tone={p().tone}>
                  <i />
                  {p().label}
                </span>
              </div>
            )}
          </Show>
          <Show
            when={props.question}
            fallback={
              <Show
                when={props.live}
                fallback={
                  <>
                    <p data-kit="cv-title">{props.title}</p>
                    <Show when={props.summary}>
                      <p data-kit="cv-sum">{props.summary}</p>
                    </Show>
                  </>
                }
              >
                <p data-kit="cv-live">{props.live}</p>
              </Show>
            }
          >
            <div data-kit="cv-asks">
              <p data-kit="cv-q">{props.question}</p>
            </div>
          </Show>
          <div data-kit="cv-acts">
            <For each={props.actions}>
              {(a) => (
                <button type="button" data-kit="cv-ghost" data-on={flag(a.on)} aria-pressed={a.on === undefined ? undefined : a.on} onClick={() => a.onClick()}>
                  <Icon name={a.icon} />
                  <span data-text="control">{a.label}</span>
                </button>
              )}
            </For>
          </div>
          {props.extra}
        </div>
      </div>
    </article>
  );
}

/** v1 .cv-quiet: check-ins that found nothing, one faint line. */
export function CrewQuiet(props: { readonly label: string; readonly time: string }): JSX.Element {
  return (
    <div data-kit="cv-quiet" data-part="crew.quiet">
      <Icon name="clock" />
      <span>{props.label}</span>
      <time>{props.time}</time>
    </div>
  );
}

/** v1 .cv-empty: nothing done yet. `lines` are the two sentences; `code` the @handle in the second. */
export function CrewEmpty(props: { readonly face?: JSX.Element; readonly name: string; readonly first: string; readonly second: readonly [string, string, string] }): JSX.Element {
  return (
    <div data-kit="cv-empty" data-part="crew.empty">
      {props.face}
      <p>
        <b>{props.name}</b> {props.first}
      </p>
      <p>
        {props.second[0]}
        <code>{props.second[1]}</code>
        {props.second[2]}
      </p>
    </div>
  );
}

/** v1 .cv-facts: how it works, one line each. */
export function CrewFacts(props: { readonly items: readonly { readonly icon: IconName; readonly text: string; readonly logo?: string | undefined; readonly onClick?: (() => void) | undefined }[] }): JSX.Element {
  return (
    <div data-kit="cv-facts" data-part="crew.facts">
      <For each={props.items}>
        {(m) => {
          const inner = (): JSX.Element => (
            <>
              <Show when={m.logo} fallback={<Icon name={m.icon} />}>
                {(src) => (
                  <i data-kit="cv-logo">
                    <img src={src()} alt="" />
                  </i>
                )}
              </Show>
              <span title={m.text}>{m.text}</span>
            </>
          );
          return (
            <Show when={m.onClick} fallback={<div data-kit="cv-fact">{inner()}</div>}>
              {(go) => (
                <button type="button" data-kit="cv-fact" data-go="" onClick={() => go()()}>
                  {inner()}
                  <Icon name="chevron" />
                </button>
              )}
            </Show>
          );
        }}
      </For>
    </div>
  );
}

/** v1 .cv-actions: Check in now, Edit…, and Remove as its icon at the end. */
export function CrewSideActions(props: { readonly items: readonly { readonly icon: IconName; readonly label: string; readonly danger?: boolean; readonly act: Act }[] }): JSX.Element {
  return (
    <div data-kit="cv-actions" data-part="crew.actions">
      <For each={props.items}>
        {(a) => (
          <WithAct
            act={a.act}
            render={(trigger) => (
              <button type="button" {...trigger()} data-kit="cv-ghost" data-danger={flag(a.danger)} title={a.label} aria-label={a.label}>
                <Icon name={a.icon} />
                <Show when={!a.danger}>
                  <span data-text="control">{a.label}</span>
                </Show>
              </button>
            )}
          />
        )}
      </For>
    </div>
  );
}

/** v1 .cv-sec: Notebook, Brief, Skills and instructions; a link on the right of its heading. */
export function CrewSection(props: {
  readonly part: string;
  readonly title: string;
  readonly link?: { readonly label: string; readonly onClick: () => void } | undefined;
  /** how many it holds, beside the title (v1 h5 small) */
  readonly count?: number | undefined;
  /** a control at the title's end in place of the link (the kit's Add menu) */
  readonly tool?: JSX.Element;
  /** files dropped on the section (the kit's .md files, v1 .cv-kit.drop): it lights up while they're over it */
  readonly onFiles?: (files: File[]) => void;
  readonly children: JSX.Element;
}): JSX.Element {
  const [over, setOver] = createSignal(false);
  const files = (e: DragEvent): boolean => !!props.onFiles && !!e.dataTransfer?.types.includes('Files');
  return (
    <div
      data-kit="cv-sec"
      data-part={props.part}
      data-drop={flag(over())}
      onDragOver={(e) => {
        if (!files(e)) return;
        e.preventDefault();
        setOver(true);
      }}
      onDragLeave={(e) => !(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node | null) && setOver(false)}
      onDrop={(e) => {
        if (!files(e)) return;
        e.preventDefault();
        setOver(false);
        props.onFiles?.([...(e.dataTransfer?.files ?? [])]);
      }}
    >
      <h5>
        {props.title}
        <Show when={props.count}>{(n) => <small>{n()}</small>}</Show>
        <Show when={props.tool}>
          <span data-kit="cv-sec-tool">{props.tool}</span>
        </Show>
        <Show when={props.link}>
          {(l) => (
            <button type="button" data-kit="link" data-tone="accent" onClick={() => l().onClick()}>
              {l().label}
            </button>
          )}
        </Show>
      </h5>
      {props.children}
    </div>
  );
}

/** v1 .cv-none / .cv-text: a section's words. */
export function CrewText(props: { readonly text: string; readonly empty?: boolean; readonly dashed?: boolean; readonly onClick?: () => void }): JSX.Element {
  return (
    <Show when={props.dashed && props.onClick} fallback={<p data-kit={props.empty ? 'cv-none' : 'cv-text'}>{props.text}</p>}>
      <button type="button" data-kit="cv-nb-empty" onClick={() => props.onClick?.()}>
        {props.text}
      </button>
    </Show>
  );
}
