// The reply box's props (ReplyBox.tsx): the full box docked at the foot of a task and the compact one.
import type { HoldRowProps } from '../composer/HoldRow.tsx';
import { on, type JSX } from 'solid-js';
import type { Act } from './act.tsx';
import type { ReplyOptionLabels } from './ReplyOptions.tsx';
import type { LauncherTool } from './launcher-fold.ts';
import type { SendShortcut } from '../composer/send-shortcut.ts';
import type { PromptSuggest } from '../composer/suggest-keys.ts';
import type { IconName } from '../content/icons.ts';

/** v1 composer.js autosize(ta, 260): the reply's tallest before it scrolls. */
export const REPLY_MAX = 260;

/** A task held at a mode cap (a channel started it, F-chan-10): a pill after the mode, `Capped at Edits`, with
 * Lift (the user's switch above the cap). A narrow bar keeps the lock and Lift. Additive (2026-11-14). */
export interface ReplyCap {
  readonly label: string;
  readonly title: string;
  readonly lift: { readonly label: string; readonly title: string; readonly act: Act };
}

export interface ReplyBoxProps {
  readonly placeholder: string;
  readonly value: string;
  /** what's in the box after each input, with the input event (v1 #800: what a phone keyboard's clipboard put in) */
  readonly onInput: (v: string, e?: InputEvent) => void;
  readonly onSend: () => void;
  /** the box's own keys after its menus (↑ history, Shift+Tab mode): true when taken */
  readonly onKey?: (e: KeyboardEvent) => boolean;
  /** Settings → Chat → Send shortcut: when Enter sends vs inserts a new line. Additive. */
  readonly sendShortcut?: SendShortcut;
  /** the @ and / menus: what the host offers for what's typed (over the box) */
  readonly suggest?: PromptSuggest | undefined;
  /** TRY-22: the rest of a suggested follow-up after what's typed, faded behind the caret with its key ("Tab"); Tab
   *  takes it. Additive (2026-11-17). */
  readonly ghost?: { readonly rest: string; readonly key: string; /** what Tab writes: the suggestion as written */ readonly full?: string } | undefined;
  /** a paste into the box (files, a pasted task link): the host may take it */
  readonly onPaste?: (e: ClipboardEvent) => void;
  /** files dropped on the box (v1: anywhere on it); it lights up while they're over it */
  readonly onFiles?: (files: File[]) => void;
  readonly inputRef?: (el: HTMLTextAreaElement) => void;
  readonly attach: string;
  readonly attachAct?: Act;
  /** the phone app's mic (v1 .c-mic): listening shows it on, in the accent. Absent: no mic. Additive (2026-11-13). */
  readonly mic?: { readonly label: string; readonly on: boolean; readonly act: Act } | undefined;
  readonly mode: { readonly label: string; readonly title: string; readonly icon?: IconName; readonly act?: Act; readonly cap?: ReplyCap | undefined };
  /** the reply's harness (v1 composer runner): one press opens the harness picker; `open` while it shows. None where no
   *  agent of its own runs the reply (a factory run's task, ADR-0114): no harness or model chip at all. */
  readonly runner?: {
    readonly title: string;
    readonly logo?: string;
    readonly act?: Act;
    readonly open?: boolean;
    /** the model it runs on, beside the logo (docs/ux/2026-11-18-desktop.md #3: not a bare logo). Additive (2026-11-18). */
    readonly model?: string | undefined;
    /** the reply's harness past 80% of a plan window: Home's ring round its logo, its words in the chip's name (UX
     *  2026-11-18 #8). Additive (2026-11-18). */
    readonly usage?: { readonly pct: number; readonly tone: 'warn' | 'bad'; readonly title: string } | undefined;
  };
  readonly team: string;
  readonly teamAct?: Act;
  readonly goal: string;
  readonly goalOn?: boolean;
  readonly goalAct?: Act;
  /** Goal on: its condition, over the box */
  readonly goalLine?: {
    readonly label: string;
    readonly placeholder: string;
    readonly value: string;
    readonly onInput: (v: string) => void;
    readonly ref?: (el: HTMLInputElement) => void;
    /** v1 .lg-keys "Enter sets it", at the line's end (not on touch). Additive (2026-11-16). */
    readonly keys?: string;
    /** Enter in the line (v1 goal.js: the goal set, with the message if one is typed). Additive (2026-11-27). */
    readonly onEnter?: () => void;
    /** Escape closes the line, the focus back in the box (v1 goal.js). Additive (2026-11-27). */
    readonly onEscape?: () => void;
    /** a small Set at the line's end: sets the goal now, no message needed. Additive (2026-11-27). */
    readonly set?: { readonly label: string; readonly title: string; readonly onClick: () => void };
  };
  readonly later: string;
  readonly laterAct?: Act;
  /** the one footer (docs/ux/2026-11-18-desktop.md #3): `+` holds what's off (attach, Goal, Delegate, Send later) and
   *  those buttons leave the bar; what's on shows as `chips` after the harness. Additive (2026-11-18). */
  readonly plus?: { readonly label: string; readonly title: string; readonly act: Act } | undefined;
  readonly chips?: readonly LauncherTool[] | undefined;
  /** the chips fold into one More when the bar is too narrow for them, as New task's tail does (launcher-fold.ts): the
   *  one whose panel is open last, a folded chip's clear kept. Without it they never fold. Additive (2026-11-18). */
  readonly chipsMore?: { readonly label: string; readonly title: string } | undefined;
  /** v1 .c-queue: while the task works, follow-ups queue (on) or interrupt it (off); its own switch look */
  readonly queue?: { readonly label: string; readonly title: string; readonly on: boolean; readonly act: Act } | undefined;
  readonly send: string;
  readonly sendAct?: Act;
  /** what's attached and when it goes: chips over the bar */
  readonly extras?: readonly { readonly id: string; readonly icon: IconName; readonly label: string; readonly removeLabel: string; readonly onRemove: () => void }[];
  /** the bar's quiet line (v1 .c-hint): the cold cache's words, with a short form for a narrow bar, and an optional link (Compact now) */
  readonly hint?: { readonly long: string; readonly short: string; readonly title?: string; readonly link?: { readonly label: string; readonly title?: string; readonly act: Act } } | undefined;
  /** what sits inside the box over the text (v1 .c-queued: the follow-ups set for later), within its border; the box
   * grows upward with it and the text and bar stay as they are. The host fills it (an extension slot). Additive (2026-11-15). */
  readonly top?: JSX.Element;
  /** Ask near the limit (v1 .c-hold): the follow-up waits for you over the text, Send anyway / Wait until the reset / ×.
   *  Additive (2026-11-16). */
  readonly hold?: Omit<HoldRowProps, 'place' | 'box'> | undefined;
  /** the phone looks' words for the options chip and its sheet: given, a phone's bar holds attach, the chip and Send
   *  (docs/ux/2026-11-18-phone.md #1); `goal` is the goal row's short name. Additive (2026-11-18). */
  readonly phoneOptions?: (ReplyOptionLabels & { readonly goal?: string }) | undefined;
}

/** v1 .composer: the reply box under a task. It wakes (its toolbar shows) when the caret goes in, and
 * folds back to one line on a press outside it (composer.js), not when the keyboard moves on. It folds once the
 * press is let go: folding on the press itself moved what's over the box (a follow-up's ×, Send now) out from under
 * the finger before its click, so on a phone the first tap after sending did nothing. */
/** The same reply box, compact (the phone's crew story, ADR-0065 step 13b): the text and Send on one line, none of the
 * task's tools (attach, mode, harness, team, goal, later). One composer, smaller: the keys, the @ and / menus, Send's
 * nudge and its pending state are the full box's own. Additive (2026-11-15). */
export interface CompactReplyBoxProps {
  readonly compact: true;
  readonly placeholder: string;
  readonly value: string;
  readonly onInput: (v: string, e?: InputEvent) => void;
  readonly onSend: () => void;
  readonly onKey?: (e: KeyboardEvent) => boolean;
  readonly suggest?: PromptSuggest | undefined;
  readonly inputRef?: (el: HTMLTextAreaElement) => void;
  readonly send: string;
  readonly sendAct?: Act;
  readonly part?: string;
}
