// The anchored card's body (loaded apart from the first page: ./Anchored.tsx). v1 ui/popover.js and ui/confirm.js ask():
// a small card floating by the control that opened it — under it (above when there's no room below), its right edge on
// the control's (`end`) or its left (`start`), kept 8 px inside the window — or near the top when nothing opened it. A
// mousedown anywhere else closes it (the control itself doesn't count, so it can toggle), Escape closes it and puts
// focus back on the control, and a resized window places it again.
import { For, onCleanup, onMount, Show, type JSX } from 'solid-js';
import { Portal } from 'solid-js/web';
import { flag } from '../core/props.ts';
import '../css/anchored.css';
import type { AnchoredCardProps, AskCardProps } from './Anchored.tsx';

const EDGE = 8;
const GAP = 6;

/** Where the card goes: by its anchor's rect, inside a window of `vw` × `vh` (pure, for the tests). */
export function placeCard(card: { w: number; h: number }, a: DOMRectReadOnly | undefined, vw: number, vh: number, align: 'start' | 'end'): { x: number; y: number } {
  const x = a ? (align === 'end' ? a.right - card.w : a.left) : (vw - card.w) / 2;
  const below = a ? a.bottom + GAP : vh * 0.18;
  const y = a && below + card.h > vh - EDGE ? a.top - GAP - card.h : below;
  return { x: Math.round(Math.max(EDGE, Math.min(x, vw - card.w - EDGE))), y: Math.round(Math.max(EDGE, Math.min(y, vh - card.h - EDGE))) };
}

export function AnchoredCardImpl(props: AnchoredCardProps): JSX.Element {
  let node: HTMLElement | undefined;
  const place = (): void => {
    if (!node) return;
    const a = props.anchor?.isConnected ? props.anchor.getBoundingClientRect() : undefined;
    const { x, y } = placeCard({ w: node.offsetWidth, h: node.offsetHeight }, a, innerWidth, innerHeight, props.align ?? 'end');
    node.style.setProperty('--k-ac-x', `${x}px`);
    node.style.setProperty('--k-ac-y', `${y}px`);
  };
  const close = (back: boolean): void => {
    props.onClose();
    if (back && props.anchor?.isConnected) props.anchor.focus();
  };
  const outside = (e: MouseEvent): void => {
    const t = e.target as Node | null;
    if (!t || node?.contains(t) || props.anchor?.contains(t)) return;
    close(false);
  };
  const onKey = (e: KeyboardEvent): void => {
    if (e.key !== 'Escape') return;
    // only this card: nothing behind it (the page's own Escape) sees the key
    e.preventDefault();
    e.stopImmediatePropagation();
    close(true);
  };
  onMount(() => {
    place();
    // not yet: the mousedown that opened it is still going on
    requestAnimationFrame(() => document.addEventListener('mousedown', outside, true));
    document.addEventListener('keydown', onKey, true);
    addEventListener('resize', place);
    const first = node?.querySelector<HTMLElement>(props.focus ?? '[data-focus]') ?? node?.querySelector<HTMLElement>('input, textarea, button');
    first?.focus();
    if (first instanceof HTMLInputElement) first.select();
  });
  onCleanup(() => {
    document.removeEventListener('mousedown', outside, true);
    document.removeEventListener('keydown', onKey, true);
    removeEventListener('resize', place);
  });
  const attrs = () => ({ 'data-kit': 'anchored', 'data-size': props.size ?? 'ask', 'data-part': props.part ?? 'ui.anchored', role: props.role ?? 'dialog', 'aria-modal': 'false' as const, 'aria-label': props.label });
  return (
    <Portal>
      <Show
        when={props.onSubmit}
        fallback={
          <div ref={(el) => (node = el)} {...attrs()}>
            {props.children}
          </div>
        }
      >
        {(submit) => (
          <form
            ref={(el) => (node = el)}
            {...attrs()}
            onSubmit={(e) => {
              e.preventDefault();
              submit()(e);
            }}
          >
            {props.children}
          </form>
        )}
      </Show>
    </Portal>
  );
}

/** v1 ask(): the question, what it means (one line each), then Cancel, the alternative and the action. */
export function AskCardImpl(props: AskCardProps): JSX.Element {
  return (
    <>
      <p data-kit="ask-q">{props.question}</p>
      <Show when={props.more.length}>
        <p data-kit="ask-more">{props.more.join('\n')}</p>
      </Show>
      <div data-kit="ask-acts">
        <For each={props.actions}>
          {(a) => (
            <button type="button" data-kit="button" data-variant={a.primary ? 'primary' : 'ghost'} data-size="sm" data-tone={a.danger ? 'bad' : undefined} data-focus={flag(a.primary)} onClick={() => a.onClick()}>
              <span data-kit="button-content">
                <span data-kit="button-label" data-text="control">
                  {a.label}
                </span>
              </span>
            </button>
          )}
        </For>
      </div>
    </>
  );
}
