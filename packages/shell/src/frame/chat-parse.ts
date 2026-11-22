// Pure parsers for Settings → Chat values (no components, so tests import them without the kit).

import type { SendShortcut } from '@wisp/ui';

export type ToolCallVisibility = 'showAll' | 'hideSuccessful' | 'hideAll' | 'zen';
export type ChatWidth = 'comfortable' | 'wide' | 'full';
export type TimeFormat = 'locale' | '12-hour' | '24-hour';

// Chat mode is on by default: only an explicit 'off' / false brings back the Cards view
export const asChatMode = (v: unknown): boolean => v !== 'off' && v !== false;

export const asToolCallVisibility = (v: unknown): ToolCallVisibility =>
  v === 'hideSuccessful' || v === 'hideAll' || v === 'zen' ? v : 'showAll';

export const asShowThinking = (v: unknown): boolean => v !== false && v !== 'off';

export const asSendShortcut = (v: unknown): SendShortcut =>
  v === 'mod-enter-multiline' || v === 'mod-enter' ? v : 'enter';

export const asChatWidth = (v: unknown): ChatWidth => (v === 'wide' || v === 'full' ? v : 'comfortable');

export const asTimeFormat = (v: unknown): TimeFormat => (v === '12-hour' || v === '24-hour' ? v : 'locale');
