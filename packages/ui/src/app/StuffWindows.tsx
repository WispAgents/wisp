// The two windows My Stuff opens over the page, as v1 draws them (css/modals.css .dr, css/stuff.css .ap; value for
// value): the reader (ui/reader.js readDoc: a doc or code file full screen, its name and path on top, a bar that fills
// as you read, its headings down the side on a wide window, Copy and Show in folder) and an app's window
// (features/stuff.js appFrame: the app in a sandboxed frame under a bar with its name and address, Reload, New tab,
// Publish, Stop and ×; "Starting…" until it loads). Both sit 18px in from the window, the whole screen on a phone;
// Escape, × or a click beside it closes them. Words come from the caller.
import { createSignal, For, onCleanup, Show, type JSX } from 'solid-js';
import { flag } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import type { IconName } from '../content/icons.ts';
import { Markdown } from '../content/markdown/Markdown.tsx';
import { Dialog } from '../controls/Dialog.tsx';
import { AppFrame, type AppFrameController } from '../frame/AppFrame.tsx';
import '../css/stuff-windows.css';

interface WinButton {
  readonly label: string;
  readonly title?: string;
  readonly icon: IconName;
  readonly onClick: (b: HTMLButtonElement) => void;
  readonly disabled?: boolean;
  /** shown for a moment after a click (Copied) */
  readonly done?: string;
}

function Ghost(props: { readonly b: WinButton; readonly kit: string }): JSX.Element {
  const [done, setDone] = createSignal(false);
  return (
    <button
      type="button"
      data-kit={props.kit}
      data-done={flag(done())}
      title={props.b.title ?? props.b.label}
      disabled={props.b.disabled}
      onClick={(e) => {
        props.b.onClick(e.currentTarget);
        if (props.b.done) setDone(true);
      }}
      // v1's flash: the word for 1.2 s, timed by a CSS animation (win-done) rather than a timer
      onAnimationEnd={() => setDone(false)}
    >
      <Icon name={props.b.icon} />
      <span>{done() && props.b.done ? props.b.done : props.b.label}</span>
    </button>
  );
}

function CloseX(props: { readonly label: string; readonly kit: string; readonly onClose: () => void }): JSX.Element {
  return (
    <button type="button" data-kit={props.kit} data-x="" title={props.label} aria-label={props.label} onClick={() => props.onClose()}>
      <Icon name="x" />
    </button>
  );
}

export interface DocReaderProps {
  readonly open: boolean;
  readonly onClose: () => void;
  readonly title: string;
  /** the file's name or path, in mono under the title */
  readonly path?: string | undefined;
  /** "1 min read" */
  readonly minutes: string;
  readonly markdown: string;
  readonly copy: WinButton;
  readonly reveal?: WinButton | undefined;
  readonly closeLabel: string;
  /** "Contents" over the headings */
  readonly contents: string;
  /** the file is longer than what's shown */
  readonly cut?: string | undefined;
}

/** v1's reader: a markdown file (or a code file as one block) full screen to read. */
export function DocReader(props: DocReaderProps): JSX.Element {
  let scroller: HTMLDivElement | undefined;
  let bar: HTMLElement | undefined;
  const [heads, setHeads] = createSignal<readonly { id: string; text: string; lv: string }[]>([]);
  const [at, setAt] = createSignal(0);
  const els: HTMLElement[] = [];
  const onScroll = (): void => {
    if (!scroller) return;
    const max = scroller.scrollHeight - scroller.clientHeight;
    bar?.style.setProperty('--k-read', String(max > 0 ? Math.min(1, scroller.scrollTop / max) : 1));
    if (!els.length) return;
    const top = scroller.getBoundingClientRect().top + 80;
    let i = 0;
    els.forEach((h, j) => {
      if (h.getBoundingClientRect().top <= top) i = j;
    });
    setAt(i);
  };
  const page = (el: HTMLElement): void => {
    // contents: the headings, once there are enough of them to find your way by
    queueMicrotask(() => {
      const hs = [...el.querySelectorAll<HTMLElement>('h3, h4')];
      els.splice(0, els.length, ...hs);
      if (hs.length < 3) return setHeads([]);
      hs.forEach((h, i) => (h.id = `dr-h${i}`));
      setHeads(hs.map((h, i) => ({ id: `dr-h${i}`, text: h.textContent ?? '', lv: h.tagName.toLowerCase() })));
      requestAnimationFrame(onScroll);
    });
  };
  return (
    <Dialog open={props.open} onOpenChange={(o) => !o && props.onClose()} title={props.title} size="window" bare part="stuff.reader">
      <div data-kit="dr" data-part="stuff.reader.window">
        <header data-kit="dr-top">
          <span data-kit="dr-ic">
            <Icon name="file" />
          </span>
          <div data-kit="dr-name">
            <b>{props.title}</b>
            <span>
              <Show when={props.path}>
                <code>{props.path}</code>
              </Show>
              <i data-dot={flag(!!props.path)}>{props.minutes}</i>
            </span>
          </div>
          <div data-kit="dr-tools">
            <Ghost b={props.copy} kit="win-ghost" />
            <Show when={props.reveal}>{(r) => <Ghost b={r()} kit="win-ghost" />}</Show>
            <CloseX label={props.closeLabel} kit="win-ghost" onClose={props.onClose} />
          </div>
          <span data-kit="dr-progress">
            <i ref={bar} />
          </span>
        </header>
        <div
          data-kit="dr-scroll"
          ref={(el) => {
            scroller = el;
            el.addEventListener('scroll', onScroll, { passive: true });
            // the bar is full at once when there's nothing to scroll (v1 ran onScroll on the first frame)
            requestAnimationFrame(onScroll);
          }}
          tabIndex={-1}
          onClick={(e) => {
            // a click on the backdrop around the page closes it, like the overlay's own
            const k = (e.target as HTMLElement).getAttribute('data-kit');
            if (k === 'dr-scroll' || k === 'dr-layout') props.onClose();
          }}
        >
          <div data-kit="dr-layout" data-toc={flag(heads().length > 0)}>
            <Show when={heads().length > 0}>
              <nav data-kit="dr-toc">
                <span data-kit="dr-toc-label">{props.contents}</span>
                <For each={heads()}>
                  {(h, i) => (
                    <a
                      href={`#${h.id}`}
                      data-lv={h.lv}
                      data-on={flag(at() === i())}
                      onClick={(e) => {
                        e.preventDefault();
                        els[i()]?.scrollIntoView({ behavior: 'smooth', block: 'start' });
                      }}
                    >
                      {h.text}
                    </a>
                  )}
                </For>
              </nav>
            </Show>
            <article data-kit="dr-page" ref={page}>
              <Markdown source={props.markdown} part="stuff.reader.page" />
            </article>
          </div>
          <Show when={props.cut}>
            <p data-kit="dr-cut">
              <Icon name="info" />
              {props.cut}
            </p>
          </Show>
        </div>
      </div>
    </Dialog>
  );
}

export interface AppWindowProps {
  readonly open: boolean;
  readonly onClose: () => void;
  readonly title: string;
  /** v3, when it was updated */
  readonly version?: string | undefined;
  /** where it runs, once it does; until then `starting` */
  readonly url?: string | undefined;
  /** what the frame loads (a grid's preview address), default url */
  readonly src?: string | undefined;
  readonly sandbox: string;
  readonly starting: string;
  /** "Starting Hello pane…" */
  readonly waiting: string;
  readonly reload: WinButton;
  /** New tab: a link to the address */
  readonly tab?: { readonly label: string; readonly title: string } | undefined;
  readonly publish?: WinButton | undefined;
  readonly stop: WinButton;
  readonly closeLabel: string;
}

const shortUrl = (u: string): string => u.replace(/^https?:\/\//, '').replace(/\/$/, '');

/** v1's app window: an app launched inside wisp, on its own origin, under its bar. */
export function AppWindow(props: AppWindowProps): JSX.Element {
  const [ready, setReady] = createSignal(false);
  let frame: AppFrameController | undefined;
  const reload: WinButton = { ...props.reload, onClick: (b) => (frame?.reload(), props.reload.onClick(b)) };
  onCleanup(() => (frame = undefined));
  return (
    <Dialog open={props.open} onOpenChange={(o) => !o && props.onClose()} title={props.title} size="window" bare part="stuff.app">
      <div data-kit="ap" data-part="stuff.app.window" data-ready={flag(ready())}>
        <header data-kit="ap-top">
          <span data-kit="ap-ic">
            <Icon name="app" />
          </span>
          <div data-kit="ap-name">
            <b>{props.title}</b>
            <span>
              <Show when={props.version}>
                <i>{props.version}</i>
              </Show>
              <i data-kit="ap-url">{props.url ? shortUrl(props.url) : props.starting}</i>
            </span>
          </div>
          <div data-kit="ap-tools">
            <Ghost b={reload} kit="win-ghost" />
            <Show when={props.tab && props.url}>
              <a data-kit="win-ghost" href={props.url} target="_blank" rel="noopener" title={props.tab?.title}>
                <Icon name="external" />
                <span>{props.tab?.label}</span>
              </a>
            </Show>
            <Show when={props.publish}>{(p) => <Ghost b={p()} kit="win-ghost" />}</Show>
            <Ghost b={props.stop} kit="win-ghost" />
            <CloseX label={props.closeLabel} kit="win-ghost" onClose={props.onClose} />
          </div>
        </header>
        <div data-kit="ap-stage">
          <Show when={!ready()}>
            <div data-kit="ap-wait">
              <i />
              <span>{props.waiting}</span>
            </div>
          </Show>
          <Show when={props.src ?? props.url}>
            {(src) => (
              <AppFrame
                title={props.title}
                src={src()}
                sandbox={props.sandbox}
                allow="fullscreen; autoplay; clipboard-write; gamepad"
                part="stuff.app.frame"
                onLoad={() => setReady(true)}
                controller={(c) => {
                  frame = c;
                  requestAnimationFrame(() => c.focus());
                }}
              />
            )}
          </Show>
        </div>
      </div>
    </Dialog>
  );
}
