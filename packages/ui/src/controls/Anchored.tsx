// A small card floating by whatever opened it (v1 ui/popover.js, confirm.js ask()): an Allow card by the ⋯ that asked
// for it, Edit details… by its card. The kit's Popover owns its trigger; this one is anchored to any element the caller
// hands it (or near the top when there's none). Its body loads on first use (./anchored-card.tsx) and isn't in
// preloadControls: only the screens that ask need it, so the first page stays under its budget (ADR-0011).
import type { JSX } from 'solid-js';
import type { KitProps, PartName, WithChildren } from '../core/props.ts';
import { lazyPart, lazyView } from './lazy-part.ts';

export interface AnchoredCardProps extends KitProps, WithChildren {
  /** what opened it; gone or undefined: near the top of the window, centred */
  readonly anchor?: HTMLElement | undefined;
  /** its right edge on the anchor's (v1's ask, the default) or its left edge (v1's Edit details) */
  readonly align?: 'end' | 'start';
  /** ask: 340 wide (v1 .ask-pop); form: 320 wide, its fields in a column (v1 .st-edit) */
  readonly size?: 'ask' | 'form';
  readonly role?: 'dialog' | 'alertdialog';
  readonly label: string;
  /** Escape, a mousedown elsewhere: the caller removes it */
  readonly onClose: () => void;
  /** what takes focus first (default: [data-focus], else the first field or button) */
  readonly focus?: string;
  /** the card is a form (v1's Edit details): Enter or its submit button sends it */
  readonly onSubmit?: (e: SubmitEvent) => void;
  readonly part?: PartName;
}

export interface AskAction {
  readonly label: string;
  readonly primary?: boolean;
  readonly danger?: boolean;
  readonly onClick: () => void;
}

export interface AskCardProps {
  readonly question: string;
  /** one line each, kept as lines */
  readonly more: readonly string[];
  /** in order: Cancel, the alternative, the action (primary) */
  readonly actions: readonly AskAction[];
}

const part = lazyPart(() => import('./anchored-card.tsx'), { preload: false });

export const AnchoredCard: (p: AnchoredCardProps) => JSX.Element = lazyView(part, (m) => m.AnchoredCardImpl);
export const AskCard: (p: AskCardProps) => JSX.Element = lazyView(part, (m) => m.AskCardImpl);
