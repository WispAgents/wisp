// My Stuff's shelf as v1 draws it (features/stuff.js paintBody, paintCard, thumb, paintKinds, emptySearch; css/stuff.css
// value for value): the kind chips, a group with its sticky head and its grid (or list), and a card: a 4:3 preview of the
// thing itself (a picture, a video's first frame, a doc's opening as a page, a snippet's first lines, a link's site, an
// app's window, else its file type), its tools on hover (Copy, Download, ⋯, Star), then its title, one faint line of
// facts with the time at its end, a note, #tags and the task it came from. Words come from the caller.
import { For, Show, type JSX } from 'solid-js';
import { flag } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { Markdown } from '../content/markdown/Markdown.tsx';
import { highlight } from '../content/highlight.ts';
import '../css/stuff-card.css';

/** What a card's preview shows (v1 thumbBody). */
export type StuffThumb =
  | { readonly kind: 'image'; readonly src: string }
  | { readonly kind: 'video'; readonly src: string }
  | { readonly kind: 'doc'; readonly markdown: string }
  | { readonly kind: 'code'; readonly text: string; readonly lang: string }
  | { readonly kind: 'link'; readonly site: string; readonly path: string }
  | { readonly kind: 'app'; readonly name: string; readonly cover?: string | undefined; readonly initial: string; readonly excerpt?: string | undefined }
  | { readonly kind: 'ext'; readonly label: string }
  /** on its way: the kind's icon (a doc or code shows its words at once) */
  | { readonly kind: 'pending'; readonly icon: IconName };

export interface StuffTool {
  readonly label: string;
  readonly title?: string;
  readonly onClick: (button: HTMLButtonElement) => void;
}

export interface StuffCardProps {
  readonly id: string;
  /** the item's kind (app, image, video, doc, code, link, file): its look */
  readonly kind: string;
  /** the kind's icon: on the facts line, and the list view's tile */
  readonly icon: IconName;
  readonly title: string;
  /** "Picture: sunset" */
  readonly label: string;
  readonly thumb: StuffThumb;
  /** "App · 2 files · 745 B" */
  readonly facts: string;
  readonly time: { readonly label: string; readonly title: string };
  /** updated since kept: v3 */
  readonly version?: { readonly label: string; readonly title: string } | undefined;
  /** published on a grid: the pill copies its link */
  readonly pub?: { readonly label: string; readonly title?: string; readonly old?: boolean; readonly busy?: boolean; readonly onClick?: (b: HTMLButtonElement) => void } | undefined;
  readonly note?: string | undefined;
  /** a published app's data warning, read whole */
  readonly openData?: string | undefined;
  readonly tags?: readonly string[] | undefined;
  /** the task it came from: a button, disabled when the task is gone */
  readonly from?: { readonly label: string; readonly title: string; readonly onClick?: (() => void) | undefined } | undefined;
  /** saved by an agent: the spark's title */
  readonly byAgent?: string | undefined;
  readonly pending?: boolean;
  readonly starred?: boolean;
  /** a card of its own is up by it (Allow, Edit details): its tools stay shown, the ⋯ that card sits by included */
  readonly held?: boolean;
  /** an app's Launch (or Running / Starting…) on its preview */
  readonly launch?: { readonly label: string; readonly title: string; readonly running?: boolean; readonly dot?: boolean; readonly onClick: () => void } | undefined;
  readonly copy?: StuffTool | undefined;
  readonly download?: StuffTool | undefined;
  readonly more?: StuffTool | undefined;
  readonly star?: StuffTool | undefined;
  readonly onOpen: () => void;
  /** a right-click: its menu at the pointer */
  readonly onMenuAt?: (x: number, y: number) => void;
  readonly onKeyDown?: (e: KeyboardEvent) => void;
  readonly ref?: (el: HTMLElement) => void;
}

/** A thing's preview, as its card shows it (the phone tiles show it too: QA phone r3 #5). */
export function StuffThumbBody(props: { readonly thumb: StuffThumb }): JSX.Element {
  const th = props.thumb;
  switch (th.kind) {
    case 'image':
      return <img src={th.src} alt="" loading="lazy" decoding="async" draggable={false} />;
    case 'video':
      return (
        <>
          <video src={`${th.src}#t=0.1`} preload="metadata" muted playsinline loop />
          <span data-kit="st-play">
            <Icon name="play" />
          </span>
        </>
      );
    case 'doc':
      return (
        <div data-kit="st-paper">
          <Markdown source={th.markdown} part="stuff.card.doc" />
        </div>
      );
    case 'code':
      return (
        <>
          <pre data-kit="st-code">
            <code>
              <For each={th.text.split('\n').slice(0, 14)}>
                {(line, i) => (
                  <>
                    {i() > 0 ? '\n' : ''}
                    <For each={line ? highlight(line, th.lang || undefined) : [{ kind: null, text: ' ' }]}>{(tk) => (tk.kind ? <span data-tk={tk.kind}>{tk.text}</span> : tk.text)}</For>
                  </>
                )}
              </For>
            </code>
          </pre>
          <span data-kit="st-lang">{th.lang || 'text'}</span>
        </>
      );
    case 'link':
      return (
        <div data-kit="st-link">
          <Icon name="web" />
          <b>{th.site}</b>
          <Show when={th.path}>
            <span>{th.path}</span>
          </Show>
        </div>
      );
    case 'app':
      return (
        <div data-kit="st-win">
          <div data-kit="st-win-bar">
            <i />
            <i />
            <i />
            <span>{th.name}</span>
          </div>
          <Show
            when={th.cover}
            fallback={
              <div data-kit="st-win-body">
                <b>{th.initial}</b>
                <Show when={th.excerpt}>
                  <span>{th.excerpt}</span>
                </Show>
              </div>
            }
          >
            {(src) => <img src={src()} alt="" loading="lazy" decoding="async" draggable={false} />}
          </Show>
        </div>
      );
    case 'ext':
      return (
        <div data-kit="st-ext">
          <Icon name="file" />
          <b>{th.label}</b>
        </div>
      );
    case 'pending':
      return (
        <div data-kit="st-ph">
          <Icon name={th.icon} />
        </div>
      );
  }
}

function Tool(props: { readonly tool: StuffTool; readonly icon: IconName; readonly kit?: string; readonly on?: boolean; readonly menu?: boolean }): JSX.Element {
  return (
    <button
      type="button"
      data-kit={props.kit ?? 'st-tool'}
      data-on={flag(props.on)}
      title={props.tool.title ?? props.tool.label}
      aria-label={props.tool.label}
      aria-haspopup={props.menu ? 'menu' : undefined}
      aria-pressed={props.kit === 'st-star' ? !!props.on : undefined}
      onClick={(e) => {
        e.stopPropagation();
        props.tool.onClick(e.currentTarget);
      }}
    >
      <Icon name={props.icon} />
    </button>
  );
}

export function StuffCard(props: StuffCardProps): JSX.Element {
  const media = (): boolean => props.thumb.kind === 'image' || props.thumb.kind === 'video';
  return (
    <article
      ref={props.ref}
      data-kit="st-card"
      data-part="stuff.card"
      data-k={props.kind}
      data-id={props.id}
      data-pending={flag(props.pending)}
      data-starred={flag(props.starred)}
      data-held={flag(props.held)}
      tabIndex={0}
      aria-label={props.label}
      onClick={(e) => {
        if (props.pending || (e.target as Element).closest('button, a, input')) return;
        props.onOpen();
      }}
      // a right-click: the caller's menu (a ContextMenu around the shelf) opens at the pointer for this card
      onContextMenu={(e) => {
        if (props.pending) return void e.preventDefault();
        props.onMenuAt?.(e.clientX, e.clientY);
      }}
      onKeyDown={(e) => props.onKeyDown?.(e)}
      onPointerOver={(e) => {
        if (props.kind === 'video' && e.pointerType === 'mouse') (e.currentTarget.querySelector('video') as HTMLVideoElement | null)?.play().catch(() => {});
      }}
      onPointerLeave={(e) => (e.currentTarget.querySelector('video') as HTMLVideoElement | null)?.pause()}
    >
      <div data-kit="st-thumb" data-part="stuff.card.thumb">
        <Show when={!media()}>
          <span data-kit="st-glyph">
            <Icon name={props.icon} />
          </span>
        </Show>
        <StuffThumbBody thumb={props.thumb} />
        <Show when={props.launch}>
          {(l) => (
            <button
              type="button"
              data-kit="st-launch"
              data-on={flag(l().running)}
              title={l().title}
              onClick={(e) => {
                e.stopPropagation();
                l().onClick();
              }}
            >
              <Show when={!l().running} fallback={l().dot ? <i data-kit="st-dot" /> : null}>
                <Icon name="play" />
              </Show>
              <span>{l().label}</span>
              <Show when={l().running}>
                <Icon name="expand" />
              </Show>
            </button>
          )}
        </Show>
        <Show when={props.byAgent}>
          {(by) => (
            <span data-kit="st-by" title={by()}>
              <Icon name="spark" />
            </span>
          )}
        </Show>
        <Show when={props.pending}>
          <span data-kit="st-saving">
            <i />
          </span>
        </Show>
      </div>
      <Show when={!props.pending}>
        <div data-kit="st-tools" data-part="stuff.card.tools">
          <Show when={props.copy}>{(c) => <Tool tool={c()} icon="copy" />}</Show>
          <Show when={props.download}>{(d) => <Tool tool={d()} icon="download" />}</Show>
          <Show when={props.more}>{(m) => <Tool tool={m()} icon="dots" kit="st-more" menu />}</Show>
          <Show when={props.star}>{(s) => <Tool tool={s()} icon="star" kit="st-star" on={!!props.starred} />}</Show>
        </div>
      </Show>
      <div data-kit="st-meta">
        <b data-kit="st-title" data-text="control" title={props.title}>
          {props.title}
        </b>
        <span data-kit="st-sub">
          <Icon name={props.icon} />
          <span data-text="control">{props.facts}</span>
          <Show when={props.version}>
            {(v) => (
              <i data-kit="st-v" title={v().title}>
                {v().label}
              </i>
            )}
          </Show>
          <Show when={props.pub}>
            {(p) => (
              <Show
                when={p().onClick && !p().busy}
                fallback={
                  <i data-kit="st-pub" data-busy="">
                    <Icon name="web" />
                    <span>{p().label}</span>
                  </i>
                }
              >
                <button
                  type="button"
                  data-kit="st-pub"
                  data-old={flag(p().old)}
                  title={p().title}
                  onClick={(e) => {
                    e.stopPropagation();
                    p().onClick?.(e.currentTarget);
                  }}
                >
                  <Icon name="web" />
                  <span>{p().label}</span>
                </button>
              </Show>
            )}
          </Show>
          <time title={props.time.title}>{props.time.label}</time>
        </span>
        <Show when={props.note}>
          <span data-kit="st-note">{props.note}</span>
        </Show>
        <Show when={props.openData}>
          <span data-kit="st-note" data-open="">
            {props.openData}
          </span>
        </Show>
        <Show when={props.tags?.length}>
          <span data-kit="st-tags" data-layout="wrap">
            <For each={props.tags}>{(tag) => <i>{`#${tag}`}</i>}</For>
          </span>
        </Show>
        <Show when={props.from}>
          {(f) => (
            <button
              type="button"
              data-kit="st-from"
              disabled={!f().onClick}
              title={f().title}
              onClick={(e) => {
                e.stopPropagation();
                f().onClick?.();
              }}
            >
              <Icon name="agent" />
              <span>{f().label}</span>
            </button>
          )}
        </Show>
      </div>
    </article>
  );
}

/** A kind chip (v1 .sf-kind): its icon (none for All), its label, how many match. */
export function StuffKind(props: { readonly label: string; readonly count: number; readonly icon?: IconName | undefined; readonly on: boolean; readonly onPick: () => void }): JSX.Element {
  return (
    <button type="button" role="tab" data-kit="sf-kind" data-part="stuff.kind" data-on={flag(props.on)} aria-selected={props.on} onClick={() => props.onPick()}>
      <Show when={props.icon}>{(i) => <Icon name={i()} />}</Show>
      <span>{props.label}</span>
      <i>{props.count}</i>
    </button>
  );
}

/** A group of the shelf (v1 .sf-group): its sticky head (label, count, Open the task) and its cards. */
export function StuffGroup(props: { readonly label: string; readonly count: number; readonly task?: { readonly label: string; readonly onClick: () => void } | undefined; readonly children?: JSX.Element }): JSX.Element {
  return (
    <section data-kit="sf-group" data-part="stuff.group">
      <h3 data-kit="sf-head">
        <span>{props.label}</span>
        <i>{props.count}</i>
        <Show when={props.task}>
          {(tk) => (
            <button type="button" data-kit="sf-task" onClick={() => tk().onClick()}>
              {tk().label}
              <Icon name="arrow" />
            </button>
          )}
        </Show>
      </h3>
      <div data-kit="sf-grid" data-part="stuff.grid">
        {props.children}
      </div>
    </section>
  );
}

/** Nothing matches the search or kind (v1 emptySearch): the words, and Show everything. */
export function StuffNone(props: { readonly text: string; readonly clear: string; readonly onClear: () => void }): JSX.Element {
  return (
    <div data-kit="sf-none-box" data-part="stuff.none">
      <p data-kit="sf-none">
        <Icon name="search" />
        {props.text}
      </p>
      <button type="button" data-kit="sf-ghost" data-size="sm" onClick={() => props.onClear()}>
        {props.clear}
      </button>
    </div>
  );
}
