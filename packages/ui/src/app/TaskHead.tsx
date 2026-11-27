// The open task's head as v1 draws it (web/css/split.css .split-head): the title, its state and the head's actions,
// which fold to their icon and then into More as the pane narrows (design/layout.md).
import { Show, createSignal, on, type JSX } from 'solid-js';
import { ForKeyed } from '../core/keyed.tsx';
import { flag } from '../core/props.ts';
import { ActIcon, WithAct } from './act.tsx';
import { Icon } from '../content/Icon.tsx';
import type { TaskAction } from './TaskDetails.tsx';

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
