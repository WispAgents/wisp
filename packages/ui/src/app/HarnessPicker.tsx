// The harness picker (v1 runner-picker.js, launcher.css .picker): ONE two-pane popover under the runner pill. Left,
// HARNESS: Auto ("Picks per task"), then every harness with its logo and a sub-line ("4 models", "Checking…", how full
// its plan is), a dot on the one in use, the one whose pane shows lit. Right: that harness's pane: Auto's GOAL list, or
// its models (a search past six), its thinking level and, with two or more, its accounts; a harness that can't run
// shows how to set it up or sign in (`body`). 580 wide (the window less 24px), inside the launcher box when it fits,
// under the pill or above it. Under 560px the harnesses become one row of chips over the pane (v1 @media 560).
// Keys (v1 keys()): ↑↓ move in a pane (on the left the right pane follows), → or Enter go right (Enter uses the
// harness), ← goes back left, Enter picks, a letter types into the search, Escape or a click outside closes.
import { Index, Show, createSignal, onCleanup, onMount, type JSX } from 'solid-js';
import { Portal } from 'solid-js/web';
import { flag, styleOf } from '../core/props.ts';
import { Icon } from '../content/Icon.tsx';
import { ThinkingSlider, type ThinkingSliderProps } from '../controls/ThinkingSlider.tsx';
import '../css/app-picker.css';

export interface HarnessPickerRow {
  readonly id: string;
  readonly name: string;
  /** the harness's logo (an image URL); Auto draws the spark */
  readonly logo?: string | undefined;
  readonly auto?: boolean;
  readonly sub: string;
  /** the sub-line in the warning tone (a plan near its line) */
  readonly warn?: boolean;
  /** dimmed: not set up or signed out (still focusable: its pane says how to get it) */
  readonly off?: boolean;
  readonly title?: string;
}

export interface HarnessPickerOption {
  readonly id: string;
  readonly label: string;
  readonly hint?: string | undefined;
}

export interface HarnessPickerPane {
  /** the list's name ("Goal", "Model") */
  readonly label: string;
  readonly groups: readonly { readonly head?: string | undefined; readonly items: readonly HarnessPickerOption[] }[];
  readonly value: string;
  readonly onPick: (id: string) => void;
  readonly search?: { readonly placeholder: string; readonly value: string; readonly onInput: (v: string) => void } | undefined;
  /** shown when the search leaves nothing */
  readonly empty: string;
  /** a help line under the list, ending in a link (Auto's "How Auto picks") */
  readonly note?: { readonly text: string; readonly link: string; readonly onLink: () => void } | undefined;
  readonly thinking?: ThinkingSliderProps | undefined;
  readonly accounts?:
    | {
        readonly label: string;
        readonly options: readonly { readonly id: string; readonly label: string; readonly off?: boolean; readonly title?: string }[];
        readonly value: string;
        readonly onPick: (id: string) => void;
        readonly note?: string | undefined;
      }
    | undefined;
  /** Try it two ways' What's different (v1 .pk-vary): a row of segments per setting under the rest; a pick commits in place */
  readonly vary?:
    | {
        readonly label: string;
        readonly rows: readonly {
          readonly id: string;
          readonly label: string;
          readonly options: readonly { readonly id: string; readonly label: string; readonly title?: string | undefined }[];
          readonly value: string;
          readonly onPick: (id: string) => void;
        }[];
      }
    | undefined;
  /** in place of the list: how to set up or sign in to a harness that can't run yet */
  readonly body?: JSX.Element;
}

export interface HarnessPickerProps {
  /** the pill it opens from */
  readonly anchor: () => Element | null | undefined;
  readonly label: string;
  /** "Harness" */
  readonly head: string;
  readonly rows: readonly HarnessPickerRow[];
  /** the harness in use (its dot) and the dot's tooltip */
  readonly inUse: string;
  readonly inUseLabel: string;
  /** the harness whose pane shows */
  readonly focused: string;
  /** arrowed to: its pane shows, nothing is picked */
  readonly onPreview: (id: string) => void;
  /** pressed (or Enter): it's used at once, the picker stays open */
  readonly onUse: (id: string) => void;
  readonly pane: HarnessPickerPane;
  readonly onClose: () => void;
}

const EDGE = 12;
/** v1 @media (max-width: 560px): the harnesses are one row of chips */
const narrow = (): boolean => typeof matchMedia === 'function' && matchMedia('(max-width: 560px)').matches;
const GAP = 6;
/** v1: under the pill when 440px fit below it */
const ROOM = 440;
const WIDTH = 580;

/** Where the card goes (v1 runnerPicker): left edge, width, and its top (below) or bottom (above) edge. Pure. */
export function pickerPlace(
  a: { readonly left: number; readonly top: number; readonly bottom: number },
  box: { readonly right: number; readonly width: number } | undefined,
  view: { readonly w: number; readonly h: number },
): { x: number; w: number; top?: number; bottom?: number } {
  const w = Math.min(WIDTH, view.w - EDGE * 2);
  const right = Math.min(view.w - EDGE, box && box.width >= w ? box.right : view.w - EDGE);
  const x = Math.max(EDGE, Math.min(a.left, right - w));
  return a.bottom + ROOM < view.h ? { x, w, top: a.bottom + GAP } : { x, w, bottom: view.h - a.top + GAP };
}

export function HarnessPicker(props: HarnessPickerProps): JSX.Element {
  let box: HTMLDivElement | undefined;
  let input: HTMLInputElement | undefined;
  const [pos, setPos] = createSignal<{ x: number; w: number; top?: number; bottom?: number }>({ x: EDGE, w: WIDTH, top: EDGE });
  const place = (): void => {
    const el = props.anchor();
    if (!el) return;
    const a = el.getBoundingClientRect();
    const host = el.closest('[data-kit="launcher"],[data-kit="composer"]')?.getBoundingClientRect();
    setPos(pickerPlace(a, host, { w: innerWidth, h: innerHeight }));
  };
  const sideRows = (): HTMLButtonElement[] => [...(box?.querySelectorAll<HTMLButtonElement>('[data-kit="pk-h"]') ?? [])];
  const options = (): HTMLButtonElement[] => [...(box?.querySelectorAll<HTMLButtonElement>('[data-kit="pk-m"]') ?? [])];
  const focusSide = (id: string): void => sideRows().find((b) => b.dataset['id'] === id)?.focus();
  const focusOptions = (): boolean => {
    const list = options();
    const target = list.find((b) => b.getAttribute('aria-selected') === 'true') ?? list[0];
    target?.focus();
    target?.scrollIntoView?.({ block: 'nearest' });
    return target !== undefined;
  };
  const step = <T extends HTMLElement>(list: T[], d: number): T | undefined => {
    if (!list.length) return undefined;
    const i = list.indexOf(document.activeElement as T);
    const next = list[i < 0 ? (d > 0 ? 0 : list.length - 1) : (i + d + list.length) % list.length];
    next?.focus();
    next?.scrollIntoView?.({ block: 'nearest' });
    return next;
  };
  const close = (): void => {
    props.onClose();
    (props.anchor() as HTMLElement | null | undefined)?.focus?.();
  };
  const keys = (e: KeyboardEvent): void => {
    const at = document.activeElement as HTMLElement | null;
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      return close();
    }
    if (!at || !box?.contains(at)) return;
    const inSide = at.dataset['kit'] === 'pk-h';
    const inList = at.dataset['kit'] === 'pk-m';
    const inSearch = at === input;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      if (!inSide && !inList && !inSearch) return; // the slider and the accounts keep their own keys
      e.preventDefault();
      e.stopPropagation();
      const d = e.key === 'ArrowDown' ? 1 : -1;
      if (inSide) {
        const next = step(sideRows(), d);
        if (next?.dataset['id']) props.onPreview(next.dataset['id']);
      } else step(options(), d);
      return;
    }
    // the chips' row on a phone runs left to right: ←→ move along it there
    const chips = narrow();
    if ((e.key === 'ArrowLeft' || e.key === 'ArrowRight') && inSide && chips) {
      e.preventDefault();
      const next = step(sideRows(), e.key === 'ArrowRight' ? 1 : -1);
      if (next?.dataset['id']) props.onPreview(next.dataset['id']);
      return;
    }
    if (e.key === 'ArrowLeft' && (inList || (inSearch && input?.value === ''))) {
      e.preventDefault();
      return focusSide(props.focused);
    }
    if ((e.key === 'ArrowRight' || e.key === 'Enter') && inSide) {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === 'Enter') props.onUse(props.focused);
      // after the pane has drawn for it
      queueMicrotask(() => void focusOptions());
      return;
    }
    if (e.key === 'Enter' && inSearch) {
      e.preventDefault();
      const first = options()[0]?.dataset['id'];
      if (first !== undefined) props.pane.onPick(first);
      return;
    }
    if (e.key.length === 1 && e.key !== ' ' && !e.metaKey && !e.ctrlKey && !e.altKey && input && !inSearch && (inSide || inList)) input.focus(); // type-to-search
  };
  const onDown = (e: PointerEvent): void => {
    const t = e.target as Node;
    if (box?.contains(t) || props.anchor()?.contains(t)) return;
    props.onClose();
  };
  onMount(() => {
    place();
    // once the portal has put the card in the page (v1 focusModels): the pick, then the search when there is one
    queueMicrotask(() => {
      if (!focusOptions()) focusSide(props.focused);
      if (props.pane.search) input?.focus({ preventScroll: true });
    });
    addEventListener('keydown', keys, true);
    addEventListener('pointerdown', onDown, true);
    addEventListener('resize', place);
  });
  onCleanup(() => {
    removeEventListener('keydown', keys, true);
    removeEventListener('pointerdown', onDown, true);
    removeEventListener('resize', place);
  });
  const p = (): HarnessPickerPane => props.pane;
  const count = (): number => p().groups.reduce((n, g) => n + g.items.length, 0);
  return (
    // inside the dialog it opens from, if any (its focus trap keeps the keys inside it), else on the page
    <Portal mount={props.anchor()?.closest('[role="dialog"]') ?? document.body}>
      <div
        ref={(el) => (box = el)}
        data-kit="picker"
        data-part="runner.picker"
        data-up={flag(pos().bottom !== undefined)}
        role="dialog"
        aria-label={props.label}
        style={styleOf({ '--k-x': `${pos().x}px`, '--k-w': `${pos().w}px`, '--k-top': pos().top === undefined ? 'auto' : `${pos().top}px`, '--k-bottom': pos().bottom === undefined ? 'auto' : `${pos().bottom}px` })}
      >
        <div data-kit="pk-side" role="listbox" aria-label={props.head} aria-orientation={narrow() ? 'horizontal' : 'vertical'}>
          <div data-kit="pk-head" aria-hidden="true">
            {props.head}
          </div>
          {/* by place, not by object: a row updated in place (a harness settling, ADR-0066) keeps its node and its focus */}
          <Index each={props.rows}>
            {(r) => (
              <button
                type="button"
                data-kit="pk-h"
                data-part="runner.harness"
                data-id={r().id}
                role="option"
                aria-selected={r().id === props.focused ? 'true' : 'false'}
                data-on={flag(r().id === props.focused)}
                data-off={flag(r().off)}
                title={r().title ?? `${r().name} · ${r().sub}`}
                tabIndex={r().id === props.focused ? 0 : -1}
                onClick={() => props.onUse(r().id)}
              >
                <span data-kit="pk-logo">
                  <Show when={!r().auto} fallback={<Icon name="spark" />}>
                    <Show when={r().logo}>{(src) => <img src={src()} alt="" />}</Show>
                  </Show>
                </span>
                <span data-kit="pk-text">
                  <b data-text="control">{r().name}</b>
                  <small data-warn={flag(r().warn)}>{r().sub}</small>
                </span>
                <Show when={r().id === props.inUse}>
                  <span data-kit="pk-dot" title={props.inUseLabel} />
                </Show>
              </button>
            )}
          </Index>
        </div>
        <div data-kit="pk-main">
          <Show when={p().search}>
            {(s) => (
              <label data-kit="pk-search">
                <Icon name="search" />
                <input ref={(el) => (input = el)} spellcheck={false} autocomplete="off" placeholder={s().placeholder} aria-label={s().placeholder} value={s().value} onInput={(e) => s().onInput(e.currentTarget.value)} />
              </label>
            )}
          </Show>
          <div data-kit="pk-list">
            <Show when={p().body === undefined} fallback={<div data-kit="pk-setup">{p().body}</div>}>
              <Index each={p().groups}>
                {(g) => (
                  <div data-kit="pk-group" role="listbox" aria-label={g().head || p().label}>
                    <Show when={g().head}>
                      <div data-kit="pk-head" aria-hidden="true">
                        {g().head}
                      </div>
                    </Show>
                    <Index each={g().items}>
                      {(m) => (
                        <button type="button" data-kit="pk-m" data-part="runner.option" data-id={m().id} role="option" aria-selected={m().id === p().value ? 'true' : 'false'} onClick={() => p().onPick(m().id)}>
                          <span data-kit="pk-text">
                            <b data-text="control">{m().label}</b>
                            <small>{m().hint ?? ''}</small>
                          </span>
                          <Icon name="check" />
                        </button>
                      )}
                    </Index>
                  </div>
                )}
              </Index>
              <Show when={count() === 0}>
                <div data-kit="pk-empty">{p().empty}</div>
              </Show>
              <Show when={p().note}>
                {(n) => (
                  <p data-kit="pk-note">
                    {n().text}{' '}
                    <a
                      href="#"
                      onClick={(e) => {
                        e.preventDefault();
                        n().onLink();
                      }}
                    >
                      {n().link}
                    </a>
                  </p>
                )}
              </Show>
            </Show>
          </div>
          <Show when={p().body === undefined && p().thinking}>
            {(th) => (
              <div data-kit="pk-effort">
                <ThinkingSlider {...th()} />
              </div>
            )}
          </Show>
          <Show when={p().body === undefined && p().accounts}>
            {(acc) => (
              <div data-kit="pk-effort" data-acct="" role="radiogroup" aria-label={acc().label}>
                <span data-kit="pk-ehead">{acc().label}</span>
                <span data-kit="pk-seg">
                  <Index each={acc().options}>
                    {(o) => (
                      <button
                        type="button"
                        role="radio"
                        aria-checked={o().id === acc().value ? 'true' : 'false'}
                        aria-disabled={o().off ? 'true' : undefined}
                        data-off={flag(o().off)}
                        title={o().title}
                        onClick={() => !o().off && acc().onPick(o().id)}
                      >
                        <span data-text="control">{o().label}</span>
                      </button>
                    )}
                  </Index>
                </span>
                <Show when={acc().note}>
                  <p data-kit="pk-anote">{acc().note}</p>
                </Show>
              </div>
            )}
          </Show>
          <Show when={p().body === undefined && p().vary}>
            {(v) => (
              <div data-kit="pk-vary" data-part="runner.vary" role="group" aria-label={v().label}>
                <div data-kit="pk-head" aria-hidden="true">
                  {v().label}
                </div>
                <Index each={v().rows}>
                  {(r) => (
                    <div data-kit="pk-effort" role="radiogroup" aria-label={r().label} data-id={r().id}>
                      <span data-kit="pk-ehead">{r().label}</span>
                      <span data-kit="pk-seg">
                        <Index each={r().options}>
                          {(o) => (
                            <button type="button" role="radio" aria-checked={o().id === r().value ? 'true' : 'false'} title={o().title} onClick={() => r().onPick(o().id)}>
                              <span data-text="control">{o().label}</span>
                            </button>
                          )}
                        </Index>
                      </span>
                    </div>
                  )}
                </Index>
              </div>
            )}
          </Show>
        </div>
      </div>
    </Portal>
  );
}
