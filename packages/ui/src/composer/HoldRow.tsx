// HoldRow: Ask near the limit's row in a prompt box (v1 features/hold.js `showHold`, css/hold.css .l-hold/.c-hold):
// why the send waits ("Claude Code's weekly window is at 91%. This task may take it to 96%."), Start anyway (Send
// anyway on a follow-up), Wait until the reset when it's known, and × Keep editing. The draft stays in the box all
// along. While it shows, the box's keys answer it (v1): Enter goes anyway (not a held-down repeat, and not in its first
// 300 ms, so a double Enter doesn't slip through), Alt+Enter waits, Esc keeps editing.
import { onCleanup, onMount, Show, type JSX } from 'solid-js';
import { sub, type KitProps, type PartName } from '../core/props.ts';
import { Button, IconButton } from '../controls/Button.tsx';
import { Wrap } from '../layout/Stack.tsx';

export interface HoldRowProps extends KitProps {
  /** the sentence: whose window, where it is, where this would take it */
  readonly say: string;
  /** "Start anyway" (New task) or "Send anyway" (a follow-up) */
  readonly goLabel: string;
  readonly goTitle?: string;
  /** "Wait until 21:00": shown only when the reset is known and more than a minute away */
  readonly wait?: { readonly label: string; readonly title?: string } | undefined;
  readonly keepLabel: string;
  readonly onGo: () => void;
  readonly onWait?: () => void;
  readonly onKeep: () => void;
  /** the box whose keys answer the row while it shows */
  readonly box?: () => HTMLTextAreaElement | undefined;
  /** `launcher`: under New task's bar (v1 .l-hold); `reply`: over the reply box's text (v1 .c-hold) */
  readonly place: 'launcher' | 'reply';
}

/** How long after it shows Enter can't go anyway (v1: a double Enter doesn't slip through). */
export const HOLD_ENTER_GRACE_MS = 300;

export function HoldRow(props: HoldRowProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.hold';
  const shown = performance.now();
  const keys = (e: KeyboardEvent): void => {
    if (e.isComposing) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopImmediatePropagation();
      props.onKeep();
      return;
    }
    if (e.key !== 'Enter' || e.shiftKey) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (e.altKey) {
      if (props.wait && props.onWait) props.onWait();
      return;
    }
    if (e.repeat || performance.now() - shown < HOLD_ENTER_GRACE_MS) return;
    props.onGo();
  };
  onMount(() => {
    const el = props.box?.();
    el?.addEventListener('keydown', keys, true);
    onCleanup(() => el?.removeEventListener('keydown', keys, true));
  });
  return (
    <div data-kit="hold" data-place={props.place} data-part={part()} role="group" aria-label={props.say} aria-live="polite">
      <p data-kit="hold-say" data-part={sub(part(), 'say')}>
        {props.say}
      </p>
      <Wrap part={sub(part(), 'acts')} gap="4">
        <Button variant="primary" size="sm" title={props.goTitle ?? props.goLabel} onClick={() => props.onGo()} part={sub(part(), 'go')}>
          {props.goLabel}
        </Button>
        <Show when={props.wait}>
          {(w) => (
            <Button variant="ghost" size="sm" icon="clock" title={w().title ?? w().label} onClick={() => props.onWait?.()} part={sub(part(), 'wait')}>
              {w().label}
            </Button>
          )}
        </Show>
      </Wrap>
      <span data-kit="hold-x">
        <IconButton icon="x" size="sm" variant="ghost" label={props.keepLabel} title={props.keepLabel} onClick={() => props.onKeep()} part={sub(part(), 'keep')} />
      </span>
    </div>
  );
}
