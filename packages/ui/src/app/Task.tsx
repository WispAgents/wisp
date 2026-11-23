// The open task as v1 draws it in the main pane (web/css/split.css .split-head, panel.css .it-body,
// .it-bar, .it-actions, transcript.css .turn/.ask/.trail, ship.css .overlap-strip, composer.css):
// its head, then its turns with the details column beside them, and the reply box docked at the foot.
import { HoldRow, type HoldRowProps } from '../composer/HoldRow.tsx';
import { For, Index, Show, createEffect, createMemo, createSignal, lazy, on, onCleanup, onMount, type JSX } from 'solid-js';
import { ForKeyed } from '../core/keyed.tsx';
import { flag } from '../core/props.ts';
import { ActIcon, WithAct, type Act } from './act.tsx';
import { ReplyOptions, type ReplyOptionLabels } from './ReplyOptions.tsx';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { useT } from '../core/i18n.tsx';
import { Menu, SheetMenus } from '../controls/Menu.tsx';
import { useTurnMenu, type TurnPhoneMenu, type TurnTool } from './turn-menu.ts';
import { holdMedia, type Held } from './hold-view.ts';
import { createTimer } from '../blocks/content-scope.ts';
import { useBarLabels } from './bar-labels.ts';
import { Tool } from './launcher-tool.tsx';
import { foldOrder, foldTail, type LauncherTool } from './launcher-fold.ts';
import { useTailFold } from './launcher-fold-measure.ts';
import { Icon } from '../content/Icon.tsx';
import { IconButton } from '../controls/Button.tsx';
import { Wrap } from '../layout/Stack.tsx';
import { growTextarea } from '../core/autosize.ts';
import { PromptSuggestList, dropHandlers } from '../composer/PromptSuggest.tsx';
import { shouldSendOnKey, type SendShortcut } from '../composer/send-shortcut.ts';
import { suggestKeyDown, type PromptSuggest } from '../composer/suggest-keys.ts';
import type { IconName } from '../content/icons.ts';
import { TaskUsage, type TaskUsageProps } from './TaskUsage.tsx';
import '../css/transcript.css';
// the full-screen picture viewer, loaded on the first click on a sent picture (it is the answers' viewer too)
const MediaViewer = lazy(() => import('../blocks/Viewer.tsx').then((m) => ({ default: m.MediaViewer })));
import '../css/app-usage.css';
import '../css/rate.css';

/** v1 composer.js autosize(ta, 260): the reply's tallest before it scrolls. */
export const REPLY_MAX = 260;

export interface TaskHeadProps {
  readonly title: string;
  readonly color: string;
  readonly renameLabel: string;
  readonly panelLabel: string;
  readonly closeLabel: string;
  /** stack only: back to the list */
  readonly backLabel?: string;
  readonly onBack?: () => void;
  readonly onPanel?: () => void;
  readonly onClose?: () => void;
  /** the side panel is open */
  readonly panelOn?: boolean;
  /** what plugins put in the head (v1 SLOTS split.head), before its own buttons */
  readonly actions?: JSX.Element;
  /** rename in place (v1 split.js): a click on the title edits it; Enter keeps, Escape or an empty title cancels */
  readonly onRename?: (title: string) => void;
  /** the task's actions, one bar in the head with and without the side panel (docs/ux/2026-11-18-desktop.md #4):
   *  Changes +n −n, Ship ▾, Done, ⋯; after what plugins put there, before the panel toggle. A narrow pane keeps their
   *  icons (the words in the tooltip). Additive (2026-11-18). */
  readonly bar?: readonly HeadAction[];
  /** Settings → Chat → Chat width: aligns the head's horizontal inset with the body. Additive. */
  readonly chatWidth?: 'comfortable' | 'wide' | 'full';
}

/** One of the head's actions: a TaskAction, with a diff's figures after its words (Changes) or a ▾ (a menu). */
export interface HeadAction extends TaskAction {
  readonly figures?: { readonly plus: string; readonly minus: string };
  /** its words show only in its tooltip (⋯) */
  readonly bare?: boolean;
}

/** The head's actions (TaskHead `bar`): made once each, by id, as the details' were (B31). */
function HeadBar(props: { readonly items: readonly HeadAction[] }): JSX.Element {
  return (
    <ForKeyed each={props.items}>
      {(a) => (
        <WithAct
          act={a().act}
          render={(trigger) => (
            <button
              type="button"
              {...trigger()}
              data-kit="head-act"
              data-a={a().id}
              data-part={a().part}
              data-tone={a().tone}
              data-key={a().key}
              data-bare={flag(a().bare)}
              data-on={flag(a().on)}
              aria-pressed={a().on === undefined ? undefined : a().on}
              aria-label={a().label}
              title={a().title}
            >
              <ActIcon act={a().act}>
                <Icon name={a().icon} />
              </ActIcon>
              <Show when={!a().bare}>
                <span data-text="control">{a().label}</span>
              </Show>
              <Show when={a().figures}>
                {(f) => (
                  <span data-kit="ha-figs">
                    <span data-kit="plus">{f().plus}</span> <span data-kit="minus">{f().minus}</span>
                  </span>
                )}
              </Show>
              <Show when={a().act?.menu && !a().bare}>
                <span data-kit="ha-caret">
                  <Icon name="chevron" size="xs" />
                </span>
              </Show>
            </button>
          )}
        />
      )}
    </ForKeyed>
  );
}

/** v1 #splitHead: the dot, the title, the side panel and close buttons (back on a phone). */
export function TaskHead(props: TaskHeadProps): JSX.Element {
  const [editing, setEditing] = createSignal(false);
  return (
    <header data-kit="split-head" data-part="split.head" data-color={props.color} data-width={props.chatWidth} data-drag="">
      <div data-kit="sh-row">
        <Show when={props.onBack} fallback={<span data-kit="q-dot" />}>
          <button type="button" data-kit="sh-back" data-part="split.back" aria-label={props.backLabel} title={props.backLabel} onClick={() => props.onBack?.()}>
            <Icon name="back" />
          </button>
        </Show>
        <h2
          data-kit="sh-title"
          data-part="split.title"
          title={props.renameLabel}
          // the Mac app: a press that moves drags the window, a click renames (audit A #18)
          data-drag-click=""
          contentEditable={editing() ? 'plaintext-only' : undefined}
          aria-label={editing() ? props.renameLabel : undefined}
          onClick={(e) => {
            if (!props.onRename || editing()) return;
            setEditing(true);
            const el = e.currentTarget;
            queueMicrotask(() => {
              el.focus();
              const r = document.createRange();
              r.selectNodeContents(el);
              const sel = window.getSelection();
              sel?.removeAllRanges();
              sel?.addRange(r);
            });
          }}
          onKeyDown={(e) => {
            if (!editing()) return;
            if (e.key === 'Enter') {
              e.preventDefault();
              e.currentTarget.blur();
            } else if (e.key === 'Escape') {
              e.currentTarget.textContent = props.title;
              setEditing(false);
              e.currentTarget.blur();
            }
          }}
          onBlur={(e) => {
            if (!editing()) return;
            setEditing(false);
            const next = (e.currentTarget.textContent ?? '').replace(/\s+/g, ' ').trim();
            if (next && next !== props.title) props.onRename?.(next);
            else e.currentTarget.textContent = props.title;
          }}
        >
          {props.title}
        </h2>
        <span data-kit="sh-acts">
          {props.actions}
          <Show when={props.bar}>{(b) => <HeadBar items={b()} />}</Show>
          <Show when={!props.onBack}>
            <button type="button" data-kit="head-btn" aria-label={props.panelLabel} title={props.panelLabel} aria-pressed={props.panelOn === true} onClick={() => props.onPanel?.()}>
              <Icon name="panel" />
            </button>
            <button type="button" data-kit="head-btn" aria-label={props.closeLabel} title={props.closeLabel} onClick={() => props.onClose?.()}>
              <Icon name="x" />
            </button>
          </Show>
        </span>
      </div>
    </header>
  );
}

export interface TaskAction {
  readonly id: string;
  readonly icon: IconName;
  readonly label: string;
  readonly title: string;
  readonly part?: string;
  readonly tone?: 'good';
  /** its key, shown at the row's end */
  readonly key?: string;
  /** desktop only: on a phone it sits in More */
  readonly deskOnly?: boolean;
  readonly act?: Act;
  /** what it shows is open (v1 .ghost.tx-toggle.on: the Transcript action while the transcript shows). Additive. */
  readonly on?: boolean;
}

/** One folder the task works in, on its page (TaskDetails `places`). */
export interface TaskPlaceChip {
  readonly name: string;
  readonly color: string;
  /** its path, and whether the task starts there or also works in it */
  readonly title?: string;
  /** a folder it also works in (not the one it started in) */
  readonly also?: boolean;
  /** opens that project; none for a folder in no project */
  readonly onOpen?: () => void;
}

export interface TaskDetailsProps {
  readonly harness: string;
  readonly logo?: string;
  readonly project: string;
  readonly color: string;
  /**
   * every folder the task works in (v1 places.js), its own first: one chip each, a click opens that project. When set it
   * replaces `project`/`color`. Additive (F-task-27).
   */
  readonly places?: readonly TaskPlaceChip[];
  /** the state, shown on a phone (the list says it beside the task on a desktop) */
  readonly status?: { readonly label: string; readonly tone: string };
  /** a plan's when (v1 plan-view .pv-when): Starts in 2h, At 3pm, Draft, After “x”; first in the column */
  readonly when?: { readonly icon: IconName; readonly label: string; readonly title?: string };
  readonly branch?: { readonly head: string; readonly tail: string; readonly title: string };
  /** its usage, its subagents' in, and how full its context is (v1 #785): the wide pane's lines. Additive. */
  readonly usage?: TaskUsageProps;
  readonly actions: readonly TaskAction[];
  /**
   * what plugins pin to the column (`shell.task:details`: wisp.ship's PR card), last in it so it stays in sight while the
   * turns scroll; a full row under the narrow pane's bar. Additive.
   */
  readonly extra?: JSX.Element;
}

/** v1 .it-bar: the harness and project, the branch, then the task's actions. */
export function TaskDetails(props: TaskDetailsProps): JSX.Element {
  return (
    <div data-kit="it-bar" data-part="split.details">
      <div data-kit="it-props">
        <Show when={props.when}>
          {(w) => (
            <span data-kit="it-when" data-part="split.when" title={w().title}>
              <Icon name={w().icon} />
              <span>{w().label}</span>
            </span>
          )}
        </Show>
        <Show when={props.status}>
          {(st) => (
            <span data-kit="it-state" data-tone={st().tone}>
              <i />
              {st().label}
            </span>
          )}
        </Show>
        <span data-kit="model-logo" role="img" aria-label={props.harness} title={props.harness}>
          <Show when={props.logo}>{(src) => <img src={src()} alt="" />}</Show>
          <span data-kit="ml-name">{props.harness}</span>
        </span>
        <Show
          when={props.places}
          fallback={
            <span data-kit="q-proj" data-color={props.color}>
              {props.project}
            </span>
          }
        >
          {(places) => (
            // by place, each chip made once (the row changes while the task runs: B31)
            <Index each={places()}>
              {(p) => (
                <Show
                  when={p().onOpen}
                  fallback={
                    <span data-kit="q-proj" data-part="split.place" data-color={p().color} data-also={flag(p().also)} title={p().title}>
                      {p().name}
                    </span>
                  }
                >
                  {(open) => (
                    <button type="button" data-kit="q-proj" data-part="split.place" data-color={p().color} data-also={flag(p().also)} title={p().title} onClick={() => open()()}>
                      {p().name}
                    </button>
                  )}
                </Show>
              )}
            </Index>
          )}
        </Show>
        {/* after the harness and the folder, as v1's it-props (split.js paintDetails: headPills, then planLine) */}
        <Show when={props.usage}>{(u) => <TaskUsage {...u()} />}</Show>
      </div>
      <Show when={props.branch}>
        {(b) => (
          <div data-kit="it-meta" title={b().title}>
            <span data-kit="m-branch">
              <Icon name="branch" />
              <span data-kit="b-h">{b().head}</span>
              <span data-kit="b-t">{b().tail}</span>
            </span>
          </div>
        )}
      </Show>
      <Show when={props.extra}>
        <div data-kit="it-extra" data-part="split.extra">
          {props.extra}
        </div>
      </Show>
      {/* the actions moved to the head (TaskHead `bar`); a host that still hands them gets them here */}
      <Show when={props.actions.length > 0}>
      <div data-kit="it-actions" data-part="split.actions">
        {/* by id, each button made once: the task's row changes a few times a second while it runs, and each change
            hands a new list (B31: every button was taken down and put back, its hover and focus lost) */}
        <ForKeyed each={props.actions}>
          {(a) => {
            return (
              <WithAct
                act={a().act}
                render={(trigger) => (
                  <button
                    type="button"
                    {...trigger()}
                    data-kit="ghost-xs"
                    data-a={a().id}
                    data-part={a().part}
                    data-tone={a().tone}
                    data-key={a().key}
                    data-desk={flag(a().deskOnly)}
                    data-on={flag(a().on)}
                    aria-pressed={a().on === undefined ? undefined : a().on}
                    title={a().title}
                  >
                    <ActIcon act={a().act}>
                      <Icon name={a().icon} />
                    </ActIcon>
                    <span data-text="control">{a().label}</span>
                  </button>
                )}
              />
            );
          }}
        </ForKeyed>
      </div>
      </Show>
    </div>
  );
}

export interface TrailItem {
  readonly icon?: IconName;
  readonly text: string;
  readonly title?: string;
}

export interface TurnAttachment {
  readonly name: string;
  readonly src?: string;
  /** a picture (an image/* mime): drawn, not named */
  readonly image?: boolean;
}

/** v1 .ask-atts: what went with the prompt, under its words. The pictures open in the media viewer, in their order. */
function AskAttachments(props: { readonly items: readonly TurnAttachment[] }): JSX.Element {
  const [view, setView] = createSignal<number | undefined>();
  const pictures = (): { src: string; alt: string }[] => props.items.flatMap((a) => (a.image && a.src ? [{ src: a.src, alt: a.name }] : []));
  const pill = (a: TurnAttachment): JSX.Element => (
    <>
      <Icon name="file" />
      <span>{a.name}</span>
    </>
  );
  return (
    <div data-kit="ask-atts">
      <Wrap gap="4" part="turn.atts">
        <For each={props.items}>
          {(a) => (
            <Show
              when={a.image && a.src}
              fallback={
                <Show
                  when={a.src}
                  fallback={
                    <span data-kit="att-pill" data-text="control" title={a.name}>
                      {pill(a)}
                    </span>
                  }
                >
                  {(src) => (
                    <a data-kit="att-pill" data-text="control" href={src()} target="_blank" rel="noopener" title={a.name}>
                      {pill(a)}
                    </a>
                  )}
                </Show>
              }
            >
              {(src) => (
                <button type="button" data-kit="ask-att-img" title={a.name} onClick={() => setView(Math.max(0, pictures().findIndex((p) => p.src === src())))}>
                  <img src={src()} alt={a.name} loading="lazy" />
                </button>
              )}
            </Show>
          )}
        </For>
      </Wrap>
      <Show when={view() !== undefined}>
        <MediaViewer list={pictures()} at={view() ?? 0} onClose={() => setView(undefined)} />
      </Show>
    </div>
  );
}

export interface TurnViewProps {
  /** the turn's id (`data-turn`): a task's kept place is held by the turn at the top (v1 split.js). Additive (2026-11-12). */
  readonly id?: string;
  readonly prompt: string;
  /** the comments the prompt went out with, as cards over its words (v1 comments.js sentComments). Additive (2026-11-12). */
  readonly askNotes?: readonly { readonly n: number; readonly on: string; readonly removed?: boolean; readonly removedLabel?: string; readonly say: string }[];
  /** the files the prompt went out with (v1 turns.js .ask-atts): a picture as a thumbnail that opens full screen, any
   *  other file as a pill that opens it; `src` is its bytes' URL (none: only its name). Additive (2026-11-21). */
  readonly attachments?: readonly TurnAttachment[];
  readonly logo?: string;
  readonly harness: string;
  readonly time: string;
  readonly timeTitle: string;
  readonly sha?: { readonly short: string; readonly title: string };
  readonly hero?: { readonly title: string; readonly summary: string };
  /** the answer's blocks as a whole (an <Answer>): shown in the hero's place, the hero among them */
  readonly answer?: JSX.Element;
  readonly acts: readonly TrailItem[];
  readonly actsTitle: string;
  readonly stats: readonly TrailItem[];
  readonly receipt?: TrailItem;
  /** after the receipt's words, in the same item (one grid cell on a phone): what plugins add to it (the cache and
   *  plan share, ADR-0095). Additive (2026-11-17). */
  readonly receiptMore?: JSX.Element;
  /** the task's latest turn: its footer's icons always show (an older turn's on hover or focus; UX 2026-11-18 #7). Additive. */
  readonly latest?: boolean;
  readonly files?: { readonly label: string; readonly plus: string; readonly minus: string; readonly act?: Act };
  readonly tools: readonly TurnTool[];
  /** a phone's answer menu beyond the tools: Copy, Comment…, Good / Bad result (turn-menu.ts). Additive (QA r9 #2). */
  readonly phoneMenu?: TurnPhoneMenu;
  /** the steps it took, shown under the brief (the trail's counts open them) */
  /** the turn's steps; a step with `out` (its output, an error's text) folds open to it (v1 transcript.js) */
  readonly activity?: {
    readonly items: readonly { readonly icon: IconName; readonly text: string; readonly detail?: string; readonly out?: string; readonly bad?: boolean }[];
    readonly open: boolean;
    readonly onToggle: () => void;
  };
  /** the agent's notes, shown under the brief */
  readonly notes?: string;
  /** still running: what it's doing, in place of the trail */
  readonly live?: string;
  /** it hasn't run (held before it started): no trail, as v1 drew one only under a finished turn. Additive (B10). */
  readonly untold?: boolean;
  /** still running: v1's working panel (status, last steps, Stop), in place of the live line. Additive (2026-11-12). */
  readonly working?: JSX.Element;
  /** 👍 / 👎 at the trail's end (a RateButtons). Additive (2026-11-12). */
  readonly rate?: JSX.Element;
  /** under the trail bar: what a trail button opened (after 👎, the WhyForm). Additive (2026-11-12). */
  readonly trailBody?: JSX.Element;
  /** Settings → Chat → Chat mode: render as a chat conversation message instead of a presentation card. Additive. */
  readonly chat?: boolean;
}

/** v1 .turn: the ask, the answer's brief, and the trail under it. */
export function TurnView(props: TurnViewProps): JSX.Element {
  // a phone (UX 2026-11-18 phone #8): an older answer's icons fold into one ⋯ menu (the latest keeps them out), which a
  // long-press on the answer opens too (its visible twin is the ⋯), so a long transcript isn't a column of icon rows;
  // the browser draws them all, as before
  const kitT = useT();
  const menu = useTurnMenu(props);
  return (
    <article ref={menu.bind} data-kit="turn" data-turn={props.id} data-live={flag(props.live !== undefined || props.working !== undefined)} data-latest={flag(props.latest)} data-chat={flag(props.chat)}>
      <div data-kit="ask">
        <Show when={(props.askNotes ?? []).length > 0}>
          <ol data-kit="ask-notes" data-part="turn.notes">
            <For each={props.askNotes}>
              {(c) => (
                <li data-kit="ask-note">
                  <span data-kit="cmt-n">{c.n}</span>
                  <div data-kit="an-body">
                    <span data-kit="an-on">
                      <span data-kit="cmt-file">{c.on}</span>
                      <Show when={c.removed}>
                        {' '}
                        <i>{c.removedLabel}</i>
                      </Show>
                    </span>
                    <Show when={c.say}>
                      <span data-kit="an-say">{c.say}</span>
                    </Show>
                  </div>
                </li>
              )}
            </For>
          </ol>
        </Show>
        <Show when={props.prompt}>
          <div data-kit="ask-text">{props.prompt}</div>
        </Show>
        <Show when={(props.attachments ?? []).length > 0}>
          <AskAttachments items={props.attachments ?? []} />
        </Show>
        <div data-kit="ask-foot">
          <span data-kit="model-logo" role="img" aria-label={props.harness}>
            <Show when={props.logo}>{(src) => <img src={src()} alt="" />}</Show>
          </span>
          <time title={props.timeTitle}>{props.time}</time>
          <Show when={props.sha}>
            {(s) => (
              <span data-kit="cp-sha" title={s().title}>
                <Icon name="branch" />
                {s().short}
              </span>
            )}
          </Show>
        </div>
      </div>
      <Show when={props.answer}>
        <div data-kit="presentation">{props.answer}</div>
      </Show>
      <Show when={!props.answer && props.hero}>
        {(h) => (
          <div data-kit="presentation">
            <div data-kit="b-hero">
              <h2>{h().title}</h2>
              <p>{h().summary}</p>
            </div>
          </div>
        )}
      </Show>
      <Show when={props.notes}>{(n) => <p data-kit="turn-notes">{n()}</p>}</Show>
      <Show when={props.activity?.open && props.activity}>
        {(a) => (
          <ul data-kit="turn-acts">
            <For each={a().items}>
              {(it) => {
                const [open, setOpen] = createSignal(false);
                return (
                  <li data-bad={flag(it.bad)} data-fold={flag(!!it.out)}>
                    <Show
                      when={it.out}
                      fallback={
                        <>
                          <Icon name={it.icon} />
                          <span>{it.text}</span>
                          <Show when={it.detail}>{(d) => <code data-kit="turn-act-detail">{d()}</code>}</Show>
                        </>
                      }
                    >
                      <button type="button" data-kit="turn-act-fold" aria-expanded={open()} onClick={() => setOpen(!open())}>
                        <Icon name={it.icon} />
                        <span>{it.text}</span>
                        <Show when={it.detail}>{(d) => <code data-kit="turn-act-detail">{d()}</code>}</Show>
                        <Icon name="chevron" />
                      </button>
                      <Show when={open()}>
                        <pre data-kit="turn-act-out">{it.out}</pre>
                      </Show>
                    </Show>
                  </li>
                );
              }}
            </For>
          </ul>
        )}
      </Show>
      <Show when={props.working} fallback={<Show when={props.live}>{(l) => <p data-kit="turn-live">{l()}</p>}</Show>}>
        <div data-kit="turn-working">{props.working}</div>
      </Show>
      <div data-kit="trail" hidden={props.live !== undefined || props.working !== undefined || props.untold === true}>
        <div data-kit="trail-bar">
          {/* v1 renderTrail: no button when the turn did nothing (or the transcript shows its steps) */}
          <Show when={props.acts.length > 0}>
            <button type="button" data-kit="tb-acts" title={props.actsTitle} aria-expanded={props.activity?.open ?? false} onClick={() => props.activity?.onToggle()}>
              {/* by place: the counts are new objects on every change of the turn's row */}
              <Index each={props.acts}>
                {(a) => (
                  <span data-kit="tb" title={a().title}>
                    <Show when={a().icon}>{(i) => <Icon name={i()} />}</Show>
                    {a().text}
                  </span>
                )}
              </Index>
            </button>
          </Show>
          <Index each={props.stats}>
            {(s) => (
              <span data-kit="tb" data-a="stat">
                {s().text}
              </span>
            )}
          </Index>
          <Show
            when={props.receipt}
            // no count of the shell's own (stats it doesn't sum): what plugins add stands as the item (ADR-0095)
            fallback={<Show when={props.receiptMore}>{<span data-kit="tb" data-part="turn.receipt">{props.receiptMore}</span>}</Show>}
          >
            {(r) => (
              <span data-kit="tb" data-part="turn.receipt" title={r().title}>
                <span>{r().text}</span>
                {props.receiptMore}
              </span>
            )}
          </Show>
          <span data-kit="grow" />
          <Show when={props.files}>
            {(f) => (
              <WithAct
                act={f().act}
                render={(trigger) => (
                  <button type="button" {...trigger()} data-kit="link-xs">
                    {f().label} <span data-kit="plus">{f().plus}</span> <span data-kit="minus">{f().minus}</span>
                  </button>
                )}
              />
            )}
          </Show>
          <span data-kit="tb-tools">
            <For each={props.tools}>
              {(t) => (
                <WithAct
                  act={t.act}
                  render={(trigger) => (
                    <button type="button" {...trigger()} data-kit="tb-btn" data-a={t.id} data-part={t.part} data-on={flag(t.on)} aria-label={t.label} title={t.label}>
                      <ActIcon act={t.act}>
                        <Icon name={t.icon} />
                      </ActIcon>
                    </button>
                  )}
                />
              )}
            </For>
            {props.rate}
            {/* a phone's ⋯ ends the action row, never a line of its own (QA r10 #6) */}
            <Show when={menu.folds()}>
              <span data-kit="tb-more">
                <Menu trigger={{ icon: 'dots', 'aria-label': kitT('more'), variant: 'ghost', size: 'sm' }} items={menu.items()} onSelect={menu.pick} open={menu.open()} onOpenChange={menu.onOpenChange} from={menu.from} placement="top-end" part="turn.more" />
              </span>
            </Show>
          </span>
        </div>
        <Show when={props.trailBody}>
          <div data-kit="trail-body">{props.trailBody}</div>
        </Show>
      </div>
    </article>
  );
}

export interface OverlapNoteProps {
  readonly head: string;
  readonly files: string;
  readonly more?: string;
  readonly moreTitle: string;
  readonly open: string;
  readonly openTitle: string;
  readonly onOpen?: () => void;
  readonly moreAct?: Act;
  /** v1 #787 .ov-x: the note's ×, last on the row (the host keeps it closed until something changes) */
  readonly dismiss?: { readonly label: string; readonly title: string; readonly onDismiss: () => void };
}

/** v1 .overlap-strip: the first task changing the same files, above the reply box. */
export function OverlapNote(props: OverlapNoteProps): JSX.Element {
  return (
    <div data-kit="overlap-strip" data-part="card.overlapNote">
      <div data-kit="ov-row">
        <Icon name="merge" />
        <span data-kit="ov-text">
          <b>{props.head}</b> <span>{props.files}</span>
        </span>
        <Show when={props.more}>
          <WithAct
            act={props.moreAct}
            render={(trigger) => (
              <button type="button" {...trigger()} data-kit="ov-more" title={props.moreTitle}>
                {props.more}
              </button>
            )}
          />
        </Show>
        <button type="button" data-kit="ghost-xs" data-compact="" title={props.openTitle} onClick={() => props.onOpen?.()}>
          {props.open}
        </button>
        <Show when={props.dismiss}>
          {(d) => (
            <button type="button" data-kit="ov-x" data-part="card.overlapClose" aria-label={d().label} title={d().title} onClick={() => d().onDismiss()}>
              <Icon name="x" />
            </button>
          )}
        </Show>
      </div>
    </div>
  );
}

/** A task held at a mode cap (a channel started it, F-chan-10): a pill after the mode, `Capped at Edits`, with
 * Lift (the user's switch above the cap). A narrow bar keeps the lock and Lift. Additive (2026-11-14). */
export interface ReplyCap {
  readonly label: string;
  readonly title: string;
  readonly lift: { readonly label: string; readonly title: string; readonly act: Act };
}

export interface ReplyBoxProps {
  readonly placeholder: string;
  readonly value: string;
  /** what's in the box after each input, with the input event (v1 #800: what a phone keyboard's clipboard put in) */
  readonly onInput: (v: string, e?: InputEvent) => void;
  readonly onSend: () => void;
  /** the box's own keys after its menus (↑ history, Shift+Tab mode): true when taken */
  readonly onKey?: (e: KeyboardEvent) => boolean;
  /** Settings → Chat → Send shortcut: when Enter sends vs inserts a new line. Additive. */
  readonly sendShortcut?: SendShortcut;
  /** the @ and / menus: what the host offers for what's typed (over the box) */
  readonly suggest?: PromptSuggest | undefined;
  /** TRY-22: the rest of a suggested follow-up after what's typed, faded behind the caret with its key ("Tab"); Tab
   *  takes it. Additive (2026-11-17). */
  readonly ghost?: { readonly rest: string; readonly key: string; /** what Tab writes: the suggestion as written */ readonly full?: string } | undefined;
  /** a paste into the box (files, a pasted task link): the host may take it */
  readonly onPaste?: (e: ClipboardEvent) => void;
  /** files dropped on the box (v1: anywhere on it); it lights up while they're over it */
  readonly onFiles?: (files: File[]) => void;
  readonly inputRef?: (el: HTMLTextAreaElement) => void;
  readonly attach: string;
  readonly attachAct?: Act;
  /** the phone app's mic (v1 .c-mic): listening shows it on, in the accent. Absent: no mic. Additive (2026-11-13). */
  readonly mic?: { readonly label: string; readonly on: boolean; readonly act: Act } | undefined;
  readonly mode: { readonly label: string; readonly title: string; readonly icon?: IconName; readonly act?: Act; readonly cap?: ReplyCap | undefined };
  /** the reply's harness (v1 composer runner): one press opens the harness picker; `open` while it shows. None where no
   *  agent of its own runs the reply (a factory run's task, ADR-0114): no harness or model chip at all. */
  readonly runner?: {
    readonly title: string;
    readonly logo?: string;
    readonly act?: Act;
    readonly open?: boolean;
    /** the model it runs on, beside the logo (docs/ux/2026-11-18-desktop.md #3: not a bare logo). Additive (2026-11-18). */
    readonly model?: string | undefined;
    /** the reply's harness past 80% of a plan window: Home's ring round its logo, its words in the chip's name (UX
     *  2026-11-18 #8). Additive (2026-11-18). */
    readonly usage?: { readonly pct: number; readonly tone: 'warn' | 'bad'; readonly title: string } | undefined;
  };
  readonly team: string;
  readonly teamAct?: Act;
  readonly goal: string;
  readonly goalOn?: boolean;
  readonly goalAct?: Act;
  /** Goal on: its condition, over the box */
  readonly goalLine?: {
    readonly label: string;
    readonly placeholder: string;
    readonly value: string;
    readonly onInput: (v: string) => void;
    readonly ref?: (el: HTMLInputElement) => void;
    /** v1 .lg-keys "Enter sets it", at the line's end (not on touch). Additive (2026-11-16). */
    readonly keys?: string;
  };
  readonly later: string;
  readonly laterAct?: Act;
  /** the one footer (docs/ux/2026-11-18-desktop.md #3): `+` holds what's off (attach, Goal, Delegate, Send later) and
   *  those buttons leave the bar; what's on shows as `chips` after the harness. Additive (2026-11-18). */
  readonly plus?: { readonly label: string; readonly title: string; readonly act: Act } | undefined;
  readonly chips?: readonly LauncherTool[] | undefined;
  /** the chips fold into one More when the bar is too narrow for them, as New task's tail does (launcher-fold.ts): the
   *  one whose panel is open last, a folded chip's clear kept. Without it they never fold. Additive (2026-11-18). */
  readonly chipsMore?: { readonly label: string; readonly title: string } | undefined;
  /** v1 .c-queue: while the task works, follow-ups queue (on) or interrupt it (off); its own switch look */
  readonly queue?: { readonly label: string; readonly title: string; readonly on: boolean; readonly act: Act } | undefined;
  readonly send: string;
  readonly sendAct?: Act;
  /** what's attached and when it goes: chips over the bar */
  readonly extras?: readonly { readonly id: string; readonly icon: IconName; readonly label: string; readonly removeLabel: string; readonly onRemove: () => void }[];
  /** the bar's quiet line (v1 .c-hint): the cold cache's words, with a short form for a narrow bar, and an optional link (Compact now) */
  readonly hint?: { readonly long: string; readonly short: string; readonly title?: string; readonly link?: { readonly label: string; readonly title?: string; readonly act: Act } } | undefined;
  /** what sits inside the box over the text (v1 .c-queued: the follow-ups set for later), within its border; the box
   * grows upward with it and the text and bar stay as they are. The host fills it (an extension slot). Additive (2026-11-15). */
  readonly top?: JSX.Element;
  /** Ask near the limit (v1 .c-hold): the follow-up waits for you over the text, Send anyway / Wait until the reset / ×.
   *  Additive (2026-11-16). */
  readonly hold?: Omit<HoldRowProps, 'place' | 'box'> | undefined;
  /** the phone looks' words for the options chip and its sheet: given, a phone's bar holds attach, the chip and Send
   *  (docs/ux/2026-11-18-phone.md #1); `goal` is the goal row's short name. Additive (2026-11-18). */
  readonly phoneOptions?: (ReplyOptionLabels & { readonly goal?: string }) | undefined;
}

/** v1 .composer: the reply box under a task. It wakes (its toolbar shows) when the caret goes in, and
 * folds back to one line on a press outside it (composer.js), not when the keyboard moves on. It folds once the
 * press is let go: folding on the press itself moved what's over the box (a follow-up's ×, Send now) out from under
 * the finger before its click, so on a phone the first tap after sending did nothing. */
/** The same reply box, compact (the phone's crew story, ADR-0065 step 13b): the text and Send on one line, none of the
 * task's tools (attach, mode, harness, team, goal, later). One composer, smaller: the keys, the @ and / menus, Send's
 * nudge and its pending state are the full box's own. Additive (2026-11-15). */
export interface CompactReplyBoxProps {
  readonly compact: true;
  readonly placeholder: string;
  readonly value: string;
  readonly onInput: (v: string, e?: InputEvent) => void;
  readonly onSend: () => void;
  readonly onKey?: (e: KeyboardEvent) => boolean;
  readonly suggest?: PromptSuggest | undefined;
  readonly inputRef?: (el: HTMLTextAreaElement) => void;
  readonly send: string;
  readonly sendAct?: Act;
  readonly part?: string;
}

/** The reply box. Its menus open as the phone apps' sheet (SheetMenus), as the launcher's do. */
export function ReplyBox(given: ReplyBoxProps | CompactReplyBoxProps): JSX.Element {
  return <SheetMenus.Provider value={true}><ReplyBoxIn {...given} /></SheetMenus.Provider>;
}

function ReplyBoxIn(given: ReplyBoxProps | CompactReplyBoxProps): JSX.Element {
  // the tools below read the full box's props; a compact box never draws them (`compact` guards each read)
  const props = given as ReplyBoxProps & { readonly compact?: boolean; readonly part?: string };
  // TRY-22: a suggested follow-up behind the caret, where there's a Tab key (a touch screen has the chips)
  // a memo, owned by the box: its key handler reads it, and reading the caller's prop getter there (a compiled memo
  // for `ghost={a ? b : c}`) made an unowned computation on every key (QA round 2 #15, "computations created outside
  // a createRoot")
  const ghost = createMemo((): { readonly rest: string; readonly key: string; readonly full?: string } | undefined =>
    !props.compact && props.ghost?.rest && !(typeof matchMedia === 'function' && matchMedia('(hover: none)').matches) ? props.ghost : undefined);
  const [awake, setAwake] = createSignal(false);
  let box: HTMLDivElement | undefined;
  // on the click the release makes (after the pressed control's own handler: Solid's sit on document first), or
  // when the press turns into a scroll
  const fold = (): void => {
    disarm();
    setAwake(false);
  };
  const disarm = (): void => {
    document.removeEventListener('click', fold);
    document.removeEventListener('pointercancel', fold);
  };
  // a primary press only (a right-click or a scrollbar makes no click, and the next click inside would fold it)
  const outside = (e: PointerEvent): void => {
    disarm();
    if (!box || box.contains(e.target as Node) || !awake() || e.button !== 0) return;
    document.addEventListener('click', fold);
    document.addEventListener('pointercancel', fold);
  };
  onMount(() => document.addEventListener('pointerdown', outside, true));
  onCleanup(() => {
    document.removeEventListener('pointerdown', outside, true);
    disarm();
  });
  const key = (e: KeyboardEvent): void => {
    if (suggestKeyDown(props.suggest, e)) return;
    // Tab takes the suggested follow-up (Shift+Tab stays the mode's key)
    const g = ghost();
    if (g && e.key === 'Tab' && !e.shiftKey && !e.altKey && !e.metaKey && !e.ctrlKey) {
      e.preventDefault();
      const full = g.full ?? props.value + g.rest;
      props.onInput(full);
      queueMicrotask(() => input?.setSelectionRange(full.length, full.length));
      return;
    }
    if (props.onKey?.(e)) return;
    if (shouldSendOnKey(e, props.value, props.sendShortcut)) {
      e.preventDefault();
      props.onSend();
    }
  };
  // Send with nothing written: the box takes the caret and its placeholder lights up until you type
  const [nudge, setNudge] = createSignal(false);
  const [over, setOver] = createSignal(false);
  let input: HTMLTextAreaElement | undefined;
  // v1 composer.js autosize(ta, 260): the reply grows with what's typed (a draft, Ask to fix's sentence) up to 260 px,
  // then scrolls; it was one fixed line, the text running off sideways (audit F orchestration F3)
  createEffect(on(() => props.value, () => input && growTextarea(input, REPLY_MAX)));
  // the bar's words go to their icons when they don't fit (bar-labels.ts; Android 360 ran 3 px over)
  const barLabels = useBarLabels();
  // the phone looks fold the bar's options into one chip and its sheet (UX 2026-11-18 phone #1); the browser's bar stays
  const look = usePlatform();
  const phoneOpts = (): (ReplyOptionLabels & { readonly goal?: string }) | undefined => (look() !== 'web' && !props.compact ? props.phoneOptions : undefined);
  // the chips as they fit (QA r10 #1 follow-up): New task's measured fold on the reply bar, the open panel's chip last
  const chipsFold = (): boolean => props.chipsMore !== undefined && !phoneOpts();
  const chipFold = useTailFold({ tail: () => foldOrder(props.chips ?? []), on: chipsFold });
  const chips = createMemo((): readonly LauncherTool[] => {
    const more = props.chipsMore;
    // read once: the host builds its chips anew on each read, and the folded set is by identity
    const all = props.chips ?? [];
    const n = all.length;
    const k = more ? Math.min(chipFold.folded(), n) : 0;
    if (!more || k === 0) return all;
    const out = new Set(foldOrder(all).slice(n - k));
    return [...all.filter((t) => !out.has(t)), foldTail(all.filter((t) => out.has(t)), more)];
  });
  const send = (): void => {
    if (props.sendAct?.pending) return;
    if (props.value.trim()) return props.onSend();
    input?.focus();
    setNudge(true); // until the next key or the caret leaves
  };
  return (
    <div
      data-nudge={flag(nudge())}
      data-kit="reply-box"
      data-part={props.part ?? 'composer'}
      data-compact={flag(props.compact)}
      data-awake={flag(awake())}
      data-drop={flag(over())}
      ref={(el) => (box = el)}
      onFocusIn={(e) => (e.target as HTMLElement).dataset['kit'] === 'reply' && setAwake(true)}
      {...dropHandlers(() => props.onFiles, setOver)}
    >
      <div data-kit="c-top" data-part="composer.top">
        {props.top}
      </div>
      <Show when={'hold' in props ? props.hold : undefined}>{(h) => <HoldRow {...h()} place="reply" box={() => input} part="composer.hold" />}</Show>
      <div data-kit="mn-wrap">
        <PromptSuggestList suggest={props.suggest} place="above" />
        <Show when={ghost()}>
          {(g) => (
            <div data-kit="reply-ghost" data-part="composer.ghost" aria-hidden="true">
              <span data-kit="rg-typed">{props.value}</span>
              <span data-kit="rg-rest">{g().rest}</span>
              <kbd data-kit="kbd">{g().key}</kbd>
            </div>
          )}
        </Show>
        <textarea
          data-kit="reply"
          data-part="composer.input"
          rows="1"
          // an empty box showing a suggestion has no placeholder under it
          placeholder={ghost() && !props.value ? '' : props.placeholder}
          value={props.value}
          ref={(el) => ((input = el), props.inputRef?.(el))}
          onBlur={() => setNudge(false)}
          onInput={(e) => (setNudge(false), props.onInput(e.currentTarget.value, e))}
          onKeyDown={key}
          onPaste={(e) => props.onPaste?.(e)}
          aria-activedescendant={props.suggest && props.suggest.items.length > 0 ? `${props.suggest.id}-o${props.suggest.active}` : undefined}
        />
      </div>
      <Show when={props.goalLine}>
        {(g) => (
          <div data-kit="l-goal" data-in="reply">
            <span data-kit="lg-k">
              <Icon name="target" />
              <span data-text="control">{g().label}</span>
            </span>
            <input data-kit="l-goal-input" ref={(el) => g().ref?.(el)} aria-label={g().label} placeholder={g().placeholder} value={g().value} onInput={(e) => g().onInput(e.currentTarget.value)} />
            <Show when={g().keys}>
              <span data-kit="lg-keys">{g().keys}</span>
            </Show>
          </div>
        )}
      </Show>
      <Show when={(props.extras?.length ?? 0) > 0}>
        <div data-kit="l-extras">
          <For each={props.extras}>
            {(x) => (
              <span data-kit="lp-chip" data-extra="">
                <Icon name={x.icon} />
                <span data-text="control">{x.label}</span>
                <button type="button" data-kit="chip-x" aria-label={x.removeLabel} title={x.removeLabel} onClick={() => x.onRemove()}>
                  <Icon name="x" />
                </button>
              </span>
            )}
          </For>
        </div>
      </Show>
      <div
        data-kit="c-bar"
        // one measurer per bar (two undo each other near a width): the chips' fold where it folds, its squeeze the bar's
        // tight form (the mode to its icon); else the labels' own
        ref={(el) => (chipsFold() ? chipFold.ref(el) : barLabels.ref(el))}
        data-tight={flag(chipsFold() ? chipFold.squeeze() > 0 : barLabels.tight())}
      >
        <Show when={!props.compact} fallback={<span data-kit="grow" />}>
          <Show
            when={props.plus}
            fallback={
              <WithAct
                act={props.attachAct}
                render={(trigger) => (
                  <button type="button" {...trigger()} data-kit="icon-btn" data-part="composer.attach" title={props.attach} aria-label={props.attach}>
                    <ActIcon act={props.attachAct}>
                      <Icon name="clip" />
                    </ActIcon>
                  </button>
                )}
              />
            }
          >
            {(p) => (
              <WithAct
                act={p().act}
                render={(trigger) => (
                  <button type="button" {...trigger()} data-kit="icon-btn" data-a="plus" data-part="composer.plus" title={p().title} aria-label={p().label}>
                    <Icon name="plus" />
                  </button>
                )}
              />
            )}
          </Show>
          <Show when={props.mic}>
            {(m) => (
              <WithAct
                act={m().act}
                render={(trigger) => (
                  <button type="button" {...trigger()} data-kit="icon-btn" data-part="composer.mic" data-on={flag(m().on)} aria-pressed={m().on} title={m().label} aria-label={m().label}>
                    <Icon name="mic" />
                  </button>
                )}
              />
            )}
          </Show>
          {/* the phone looks (UX 2026-11-18 phone #1): the mode, the harness and the tools behind one chip and its sheet */}
          <Show
            when={!phoneOpts()}
            fallback={
              <ReplyOptions
                labels={phoneOpts() as ReplyOptionLabels}
                mode={{ label: props.mode.label, icon: props.mode.icon, act: props.mode.act }}
                {...(props.runner && { runner: { title: props.runner.title, logo: props.runner.logo, model: props.runner.model, act: props.runner.act, open: props.runner.open } })}
                rows={[
                  ...(props.plus
                    ? []
                    : [
                        { id: 'team', part: 'composer.team', icon: 'agent' as const, label: props.teamAct?.panel?.label ?? props.team, act: props.teamAct },
                        { id: 'goal', part: 'composer.goal', icon: 'target' as const, label: phoneOpts()?.goal ?? props.goal, on: props.goalOn === true, act: props.goalAct },
                        { id: 'later', part: 'composer.sendLater', icon: 'clock' as const, label: props.later, act: props.laterAct },
                      ]),
                  ...(props.queue ? [{ id: 'queue', part: 'composer.queueSwitch', icon: 'queue' as const, label: props.queue.label, on: props.queue.on, act: props.queue.act }] : []),
                ]}
              />
            }
          >
          <WithAct
            act={props.mode.act}
            render={(trigger) => (
              <button type="button" {...trigger()} data-kit="mode-btn" data-part="composer.mode" data-reply="" title={props.mode.title} aria-label={props.mode.label}>
                <Icon name={props.mode.icon ?? 'spark'} />
                <span data-text="control">{props.mode.label}</span>
              </button>
            )}
          />
          <Show when={props.mode.cap}>
            {(c) => (
              <span data-kit="c-cap" data-part="composer.cap" title={c().title}>
                <Icon name="lock" size="xs" />
                <span data-kit="c-cap-words" data-text="control">
                  {c().label}
                </span>
                <WithAct
                  act={c().lift.act}
                  render={(trigger) => (
                    <button type="button" {...trigger()} data-kit="c-cap-lift" data-part="composer.capLift" title={c().lift.title}>
                      <span data-text="control">{c().lift.label}</span>
                    </button>
                  )}
                />
              </span>
            )}
          </Show>
          <Show when={props.runner}>
            {(r) => (
          <WithAct
            act={r().act}
            render={(trigger) => (
              <button
                type="button"
                {...trigger()}
                data-kit="c-runner"
                data-part="composer.harness"
                aria-haspopup="dialog"
                aria-expanded={r().open ? 'true' : 'false'}
                title={r().usage ? `${r().title}\n${r().usage?.title ?? ""}` : r().title}
                aria-label={r().usage ? `${r().title}\n${r().usage?.title ?? ""}` : r().title}
              >
                <span data-kit="rb-logo" data-bare="">
                  <Show when={r().logo}>{(src) => <img src={src()} alt="" />}</Show>
                  <Show when={r().usage}>{(u) => <span data-kit="rb-ring" data-part="composer.harness.usage" data-tone={u().tone} aria-hidden="true" style={{ '--ctx': `${u().pct}%` }} />}</Show>
                </span>
                <Show when={r().model}>
                  {(m) => (
                    <span data-kit="c-model" data-text="control">
                      {m()}
                    </span>
                  )}
                </Show>
              </button>
            )}
          />
            )}
          </Show>
          <Show when={!props.plus}>
          <WithAct
            act={props.teamAct}
            render={(trigger) => (
              <button type="button" {...trigger()} data-kit="c-team" data-part="composer.team" title={props.team} aria-label={props.team}>
                <Icon name="agent" />
              </button>
            )}
          />
          <WithAct
            act={props.goalAct}
            render={(trigger) => (
              <button
                type="button"
                {...trigger()}
                data-kit="icon-btn"
                data-a="goal"
                data-part="composer.goal"
                data-on={flag(props.goalOn)}
                aria-pressed={props.goalOn === true}
                title={props.goal}
                aria-label={props.goal}
              >
                <Icon name="target" />
              </button>
            )}
          />
          </Show>
          {/* by chip, not by slot: an open Delegate panel keeps its node as Goal comes and goes */}
          <ForKeyed each={chips()}>{(t) => <Tool t={t()} />}</ForKeyed>
          </Show>
          <Show when={props.hint} fallback={<span data-kit="grow" />}>
            {(h) => (
              // no words, only a link (a phone's factory run: Stop the run): the link alone, whole, never cut by the
              // words' container (audit G tasks #6)
              <span data-kit="c-hint" data-part="composer.hint" data-bare={flag(!h().long && !!h().link)}>
                <span data-kit="c-cold-row">
                  <Show when={h().long}>
                    <span data-kit="c-cold" title={h().title ?? h().long}>
                      <span data-kit="cc-long">{h().long}</span>
                      <span data-kit="cc-short">{h().short}</span>
                    </span>
                  </Show>
                  <Show when={h().link}>
                    {(l) => (
                      <WithAct
                        act={l().act}
                        render={(trigger) => (
                          <button type="button" {...trigger()} data-kit="link" data-part="composer.hint.link" title={l().title ?? l().label}>
                            {l().label}
                          </button>
                        )}
                      />
                    )}
                  </Show>
                </span>
              </span>
            )}
          </Show>
          <Show when={!phoneOpts()}>
          <Show when={props.queue}>
            {(q) => (
              <WithAct
                act={q().act}
                render={(trigger) => (
                  <button type="button" {...trigger()} data-kit="c-queue" data-part="composer.queueSwitch" data-on={flag(q().on)} aria-pressed={q().on} title={q().title}>
                    <Icon name="queue" />
                    <span data-text="control">{q().label}</span>
                    <i data-kit="c-switch" />
                  </button>
                )}
              />
            )}
          </Show>
          <Show when={!props.plus}>
          <WithAct
            act={props.laterAct}
            render={(trigger) => (
              <button type="button" {...trigger()} data-kit="icon-btn" data-part="composer.sendLater" title={props.later} aria-label={props.later}>
                <Icon name="clock" />
              </button>
            )}
          />
          </Show>
          </Show>
        </Show>
        <button type="button" data-kit="send" data-part="composer.send" title={props.send} aria-label={props.send} onClick={send} aria-busy={props.sendAct?.pending ? 'true' : undefined}>
          <ActIcon act={props.sendAct}>
            <Icon name="enter" />
          </ActIcon>
        </button>
      </div>
    </div>
  );
}

/** Imperative scroll control exposed by TaskBody so the host (TaskView) coordinates follow-ups, initial landing and
 *  kept-place restore with hold-view's follow state instead of writing raw scrollTop behind its back. */
export type TaskScroll = Pick<Held, 'toEnd' | 'place' | 'following'>;

/** v1 .it-body: the task's page under its head: the details column, the turns, the dock. */
export function TaskBody(props: {
  readonly details?: JSX.Element;
  readonly turns: JSX.Element;
  readonly dock: JSX.Element;
  readonly wide?: boolean;
  /** Settings → Chat → Chat width. Additive. */
  readonly chatWidth?: 'comfortable' | 'wide' | 'full';
  /** Settings → Chat → Chat mode. Additive. */
  readonly chat?: boolean;
  /** the scroller (a task's kept place) */ readonly bodyRef?: (el: HTMLElement) => void;
  /** coordinated scroll controller (follow-ups, place restore, Chat initial landing). Additive. */
  readonly scrollRef?: (ctrl: TaskScroll) => void;
  /** Jump to latest's name: the button over the dock once the reader scrolled away from the end. Additive. */
  readonly toLatest?: string;
}): JSX.Element {
  let stop: (() => void) | undefined;
  let held: Held | undefined;
  let body!: HTMLDivElement;
  onCleanup(() => stop?.());
  // away from the end (the view no longer follows) and whether more came in since: hold-view says when it changes
  const [away, setAway] = createSignal({ away: false, fresh: false });
  // shown 150 ms after the reader left the end (t3code), gone at once when they're back; updates to `fresh` during the
  // delay piggyback on the running timer instead of restarting it
  const later = createTimer();
  let pendingAway: { readonly away: boolean; readonly fresh: boolean } | undefined;
  const onAway = (o: { readonly away: boolean; readonly fresh: boolean }): void => {
    if (o.away && !away().away) {
      const waiting = pendingAway !== undefined;
      pendingAway = o;
      if (!waiting) {
        later.after(150, () => {
          const next = pendingAway;
          pendingAway = undefined;
          if (next) setAway(next);
        });
      }
      return;
    }
    pendingAway = undefined;
    later.clear();
    setAway(o);
  };
  // at once, not smooth: a smooth scroll's steps read as the reader leaving the end again (and the click answers in
  // the frame, ADR-0017)
  const toLatest = (): void => {
    pendingAway = undefined;
    later.clear();
    setAway({ away: false, fresh: false });
    if (held) held.toEnd();
    else body.scrollTop = body.scrollHeight;
  };
  return (
    <div
      ref={(el) => {
        body = el;
        props.bodyRef?.(el);
      }}
      data-kit="it-body"
      data-part="split.body card.body"
      data-details={flag(props.details !== undefined)}
      data-width={props.chatWidth}
      data-chat={flag(props.chat)}
    >
      {props.details}
      <div data-kit="turns">{props.turns}</div>
      <div
        ref={(dock) => {
          // a child's ref runs after its parent's: the body is known here
          stop?.();
          held = typeof ResizeObserver === 'undefined' ? undefined : holdMedia(body, dock, onAway);
          if (held) props.scrollRef?.({ toEnd: held.toEnd, place: held.place, following: held.following });
          // the dock's height for what stops above it (a live question, UX 2026-11-18 #1)
          const ro = typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(() => body.style.setProperty('--dock-h', `${dock.offsetHeight}px`));
          ro?.observe(dock);
          const h = held;
          stop = () => (h?.stop(), ro?.disconnect());
        }}
        data-kit="it-dock"
      >
        <Show when={props.toLatest !== undefined && away().away}>
          <span data-kit="to-latest" data-part="task.toLatest" data-fresh={flag(away().fresh)}>
            <IconButton icon="arrow-down" variant="secondary" label={props.toLatest ?? ''} onClick={toLatest} />
          </span>
        </Show>
        {props.dock}
      </div>
    </div>
  );
}

