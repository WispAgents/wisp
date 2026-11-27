// The reply box as v1 draws it (composer.css), docked at the foot of the open task: the text, its @ and / menus,
// the tools row (attach, mode, harness, goal, later) and Send.
import { HoldRow } from '../composer/HoldRow.tsx';
import { Show, createEffect, createMemo, createSignal, on, onCleanup, onMount, type JSX } from 'solid-js';
import { ForKeyed } from '../core/keyed.tsx';
import { flag } from '../core/props.ts';
import { ActIcon } from './act.tsx';
import type { ReplyOptionLabels } from './ReplyOptions.tsx';
import { usePlatform } from '../platform/PlatformLook.tsx';
import { SheetMenus } from '../controls/Menu.tsx';
import { useBarLabels } from './bar-labels.ts';
import { Tool } from './launcher-tool.tsx';
import { foldOrder, foldTail, type LauncherTool } from './launcher-fold.ts';
import { useTailFold } from './launcher-fold-measure.ts';
import { Icon } from '../content/Icon.tsx';

import { growTextarea } from '../core/autosize.ts';
import { PromptSuggestList, dropHandlers } from '../composer/PromptSuggest.tsx';
import { shouldSendOnKey } from '../composer/send-shortcut.ts';
import { suggestKeyDown } from '../composer/suggest-keys.ts';
import { REPLY_MAX, type ReplyBoxProps, type CompactReplyBoxProps } from './reply-box-props.ts';
import {
  ReplyAttach,
  ReplyExtras,
  ReplyGoalLine,
  ReplyHint,
  ReplyMic,
  ReplyMode,
  ReplyPhoneOptions,
  ReplyQueueLater,
  ReplyRunner,
  ReplyTeamGoal,
  type BoxProps,
} from './reply-box-parts.tsx';

/** The reply box. Its menus open as the phone apps' sheet (SheetMenus), as the launcher's do. */
export function ReplyBox(given: ReplyBoxProps | CompactReplyBoxProps): JSX.Element {
  return <SheetMenus.Provider value={true}><ReplyBoxIn {...given} /></SheetMenus.Provider>;
}

function ReplyBoxIn(given: ReplyBoxProps | CompactReplyBoxProps): JSX.Element {
  // the tools below read the full box's props; a compact box never draws them (`compact` guards each read)
  const props = given as BoxProps;
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
      <ReplyGoalLine box={props} />
      <ReplyExtras box={props} />
      <div
        data-kit="c-bar"
        // one measurer per bar (two undo each other near a width): the chips' fold where it folds, its squeeze the bar's
        // tight form (the mode to its icon); else the labels' own
        ref={(el) => (chipsFold() ? chipFold.ref(el) : barLabels.ref(el))}
        data-tight={flag(chipsFold() ? chipFold.squeeze() > 0 : barLabels.tight())}
      >
        <Show when={!props.compact} fallback={<span data-kit="grow" />}>
          <ReplyAttach box={props} />
          <ReplyMic box={props} />
          {/* the phone looks (UX 2026-11-18 phone #1): the mode, the harness and the tools behind one chip and its sheet */}
          <Show
            when={!phoneOpts()}
            fallback={
              <ReplyPhoneOptions box={props} opts={phoneOpts} />
            }
          >
          <ReplyMode box={props} />
          <ReplyRunner box={props} />
          <ReplyTeamGoal box={props} />
          {/* by chip, not by slot: an open Delegate panel keeps its node as Goal comes and goes */}
          <ForKeyed each={chips()}>{(t) => <Tool t={t()} />}</ForKeyed>
          </Show>
          <ReplyHint box={props} />
          <Show when={!phoneOpts()}>
          <ReplyQueueLater box={props} />
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
