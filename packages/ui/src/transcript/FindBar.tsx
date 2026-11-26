// FindBar: Find in a task (mod+f, R4-3), one line pinned over the turns: the words, "n of m", up and down, close.
// Enter goes to the next hit, Shift+Enter to the one before, Escape closes (the caller gives focus back). It finds
// nothing itself: what matches is the caller's, from the steps it holds, never the page's text.
import { onMount, type JSX } from 'solid-js';
import { Icon } from '../content/Icon.tsx';
import { IconButton } from '../controls/Button.tsx';
import { flag, sub, type KitProps, type PartName } from '../core/props.ts';
import '../css/find.css';

export interface FindBarProps extends KitProps {
  readonly value: string;
  readonly onInput: (value: string) => void;
  /** "3 of 12", "No results": what the words found, beside them */
  readonly count?: string;
  /** still asking for the older turns' hits */
  readonly busy?: boolean;
  readonly onNext: () => void;
  readonly onPrev: () => void;
  readonly onClose: () => void;
  readonly labels: { readonly placeholder: string; readonly next: string; readonly prev: string; readonly close: string };
}

export function FindBar(props: FindBarProps): JSX.Element {
  const part = (): PartName => props.part ?? 'ui.find';
  let input: HTMLInputElement | undefined;
  onMount(() => {
    input?.focus();
    input?.select();
  });
  const onKey = (e: KeyboardEvent): void => {
    if (e.isComposing) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      if (e.shiftKey) props.onPrev();
      else props.onNext();
    } else if (e.key === 'Escape') {
      // the bar's own Escape: the page's Back doesn't also run (keyboard.ts skips a key already handled)
      e.preventDefault();
      props.onClose();
    }
  };
  return (
    <div id={props.id} data-kit="find-bar" data-part={part()} role="search">
      <Icon name="search" size="sm" />
      <input
        ref={input}
        type="search"
        data-part={sub(part(), 'input')}
        value={props.value}
        placeholder={props.labels.placeholder}
        aria-label={props.labels.placeholder}
        onInput={(e) => props.onInput(e.currentTarget.value)}
        onKeyDown={onKey}
      />
      <span data-kit="find-count" data-text="control" data-busy={flag(props.busy)} aria-live="polite">
        {props.count ?? ''}
      </span>
      <IconButton part={sub(part(), 'prev')} size="xs" icon="arrow-up" label={props.labels.prev} onClick={() => props.onPrev()} />
      <IconButton part={sub(part(), 'next')} size="xs" icon="arrow-down" label={props.labels.next} onClick={() => props.onNext()} />
      <IconButton part={sub(part(), 'close')} size="xs" icon="x" label={props.labels.close} onClick={() => props.onClose()} />
    </div>
  );
}
