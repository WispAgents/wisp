// Reactive readers & pure parsers for Settings → Chat (`settings.transcript`, `settings.chatToolCalls`,
// `settings.chatThinking`, `settings.sendShortcut`, `settings.chatWidth`, `settings.timeFormat`).
// Reads through `useSignal` so Solid tracks alien-signals (ARCHITECTURE.md rule 13) and flipping a
// switch in Settings updates every open task immediately without a reload.

import { entityKey, type Client, type PeerId } from '@wisp/client';
import { useSignal, type SendShortcut } from '@wisp/ui';

import { asChatMode, asChatWidth, asSendShortcut, asShowThinking, asTimeFormat, asToolCallVisibility, type ChatWidth, type TimeFormat, type ToolCallVisibility } from './chat-parse.ts';

export * from './chat-parse.ts';

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
