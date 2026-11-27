// One turn of the open task as v1 draws it (transcript.css .turn/.ask/.trail): the ask with its attachments, the
// answer's brief, the trail and the turn's tools.
import { For, Index, Show, createSignal, lazy, on, type JSX } from 'solid-js';
import { flag } from '../core/props.ts';
import { ActIcon, WithAct, type Act } from './act.tsx';
import { useT } from '../core/i18n.tsx';
import { Menu } from '../controls/Menu.tsx';
import { useTurnMenu, type TurnPhoneMenu, type TurnTool } from './turn-menu.ts';
import { Icon } from '../content/Icon.tsx';
import { Wrap } from '../layout/Stack.tsx';
import type { IconName } from '../content/icons.ts';

// the full-screen picture viewer, loaded on the first click on a sent picture (it is the answers' viewer too)
const MediaViewer = lazy(() => import('../blocks/Viewer.tsx').then((m) => ({ default: m.MediaViewer })));
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
