// The `question` block (v1 R.question): the agent's question with its numbered options. Live only on the task's
// latest answer (BlockContext.latest): a click sends the option (its own reply when it has one), and so does its
// number key while the reply box is empty (UX 2026-11-18 #1). The reply box is the other way to answer: it says "Or
// answer in your own words…" (no second control for it here). On an earlier answer it reads "Agent asked", greyed,
// its options inert.
import { onCleanup, onMount, type JSX } from 'solid-js';
import { Icon } from '../content/Icon.tsx';
import { AskOptions } from './AskOptions.tsx';
import { InlineMd } from './content-parts.tsx';
import { useBlockT, useBlocks } from './context.tsx';
import { items, opt, str } from './read.ts';
import type { BlockProps } from './types.ts';

interface Option {
  readonly label: string;
  readonly detail: string | undefined;
  readonly reply: string;
}

export function QuestionBlock(props: BlockProps): JSX.Element {
  const ctx = useBlocks();
  const t = useBlockT();
  const live = (): boolean => ctx.latest === true && !!ctx.reply;
  const options = (): Option[] =>
    items<Option>(
      props.block['options'],
      (o) => ({
        label: str(o['label']),
        detail: opt(o['detail']),
        reply: opt(o['reply']) ?? str(o['label']),
      }),
      (s) => ({ label: s, detail: undefined, reply: s }),
    ).filter((o) => o.label);
  // 1–9 pick an option while nothing is being typed: focus outside any field, or in an empty reply box
  const onKey = (e: KeyboardEvent): void => {
    if (!live() || e.metaKey || e.ctrlKey || e.altKey || e.isComposing || e.defaultPrevented || !/^[1-9]$/.test(e.key)) return;
    const at = document.activeElement as HTMLElement | null;
    // a menu, a dialog or a sheet over the page has the keys
    if (at?.closest('[role="menu"], [role="dialog"], [role="listbox"], [aria-modal="true"]') || document.querySelector('[aria-modal="true"]')) return;
    const field = !!at && (at.isContentEditable || at.tagName === 'INPUT' || at.tagName === 'TEXTAREA' || at.tagName === 'SELECT');
    if (field && !(at.closest('[data-kit="reply-box"]') && ((at as HTMLTextAreaElement).value ?? at.textContent ?? '') === '')) return;
    const o = options()[Number(e.key) - 1];
    if (!o) return;
    e.preventDefault();
    ctx.reply?.(o.reply);
  };
  onMount(() => document.addEventListener('keydown', onKey));
  onCleanup(() => document.removeEventListener('keydown', onKey));
  return (
    <div data-kit="rb-question" data-part="block.question" data-answered={live() ? undefined : ''}>
      <div data-kit="rb-q-head">
        <Icon name="ask" size="sm" />
        <span>{live() ? t('block.question.live') : t('block.question.asked')}</span>
      </div>
      <p data-kit="rb-q-text">
        <InlineMd source={str(props.block['question'])} part="block.question.text" />
      </p>
      <AskOptions options={options()} disabled={!live()} part="block.question.options" optionPart="block.question.option" onPick={(i) => {
        const o = options()[i];
        if (o) ctx.reply?.(o.reply);
      }} />
    </div>
  );
}
