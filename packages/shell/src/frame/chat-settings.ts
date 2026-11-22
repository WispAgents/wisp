// Reactive readers & pure parsers for Settings → Chat (`settings.transcript`, `settings.chatToolCalls`,
// `settings.chatThinking`, `settings.sendShortcut`, `settings.chatWidth`, `settings.timeFormat`).
// Reads through `useSignal` so Solid tracks alien-signals (ARCHITECTURE.md rule 13) and flipping a
// switch in Settings updates every open task immediately without a reload.

import { entityKey, type Client, type PeerId } from '@wisp/client';
import { useSignal, type SendShortcut } from '@wisp/ui';

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

export interface ChatSettings {
  readonly chatMode: () => boolean;
  readonly toolCalls: () => ToolCallVisibility;
  readonly showThinking: () => boolean;
  readonly sendShortcut: () => SendShortcut;
  readonly chatWidth: () => ChatWidth;
  readonly timeFormat: () => TimeFormat;
}

export function useChatSettings(client: Pick<Client, 'entities'>, peer: PeerId): ChatSettings {
  const row = (key: string) => useSignal(client.entities.row<{ readonly value?: unknown }>('setting', entityKey(peer, key)));
  const mode = row('settings.transcript');
  const tools = row('settings.chatToolCalls');
  const thinking = row('settings.chatThinking');
  const shortcut = row('settings.sendShortcut');
  const width = row('settings.chatWidth');
  const time = row('settings.timeFormat');
  return {
    chatMode: () => asChatMode(mode()?.value),
    toolCalls: () => asToolCallVisibility(tools()?.value),
    showThinking: () => asShowThinking(thinking()?.value),
    sendShortcut: () => asSendShortcut(shortcut()?.value),
    chatWidth: () => asChatWidth(width()?.value),
    timeFormat: () => asTimeFormat(time()?.value),
  };
}
