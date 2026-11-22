// Settings → Chat: Chat mode (replaces Full transcript) and the chat options modeled after t3code
// (Tool calls in chat, Show thinking, Follow-up behavior, Send shortcut, Chat width, Time format).
// wisp blocks remains an independent toggle in Settings → General so agents can still embed blocks inline.

import type { JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { ChoiceSetting, SelectSetting, SettingsGroup, SwitchSetting } from '@wisp/ui';
import { t } from '../i18n.ts';
import { fromRow, onOff, text } from '../source.ts';
import { KEYS } from '../types.ts';
import '../i18n/lazy.ts';

const locked = (by: string): string => t('shell-settings.locked', { by });

const isMac = (): boolean => typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/i.test(navigator.platform || navigator.userAgent);

export default function Chat(props: { readonly client: PluginClient }): JSX.Element {
  const mod = isMac() ? '⌘' : 'Ctrl';
  return (
    <>
      <SettingsGroup part="settings.chat.mode">
        <SwitchSetting
          title={t('shell-settings.chat-mode')}
          description={t('shell-settings.chat-mode.hint')}
          source={onOff(fromRow(props.client, KEYS.transcript), true, KEYS.transcript)}
          locked={locked}
        />
      </SettingsGroup>
      <SettingsGroup part="settings.chat.activity">
        <SelectSetting
          part="settings.chat.toolCalls"
          title={t('shell-settings.chat-tool-calls')}
          description={t('shell-settings.chat-tool-calls.hint')}
          source={text(fromRow(props.client, KEYS.chatToolCalls), 'showAll')}
          locked={locked}
          items={[
            { value: 'showAll', label: t('shell-settings.chat-tool-calls.showAll') },
            { value: 'hideSuccessful', label: t('shell-settings.chat-tool-calls.hideSuccessful') },
            { value: 'hideAll', label: t('shell-settings.chat-tool-calls.hideAll') },
            { value: 'zen', label: t('shell-settings.chat-tool-calls.zen') },
          ]}
        />
        <SwitchSetting
          title={t('shell-settings.chat-thinking')}
          description={t('shell-settings.chat-thinking.hint')}
          source={onOff(fromRow(props.client, KEYS.chatThinking), true, KEYS.chatThinking)}
          locked={locked}
        />
      </SettingsGroup>
      <SettingsGroup part="settings.chat.composer">
        <SelectSetting
          part="settings.chat.followUp"
          title={t('shell-settings.chat-follow-up')}
          description={t('shell-settings.chat-follow-up.hint')}
          source={text(fromRow(props.client, KEYS.followUp), 'interrupt')}
          locked={locked}
          items={[
            { value: 'interrupt', label: t('shell-settings.chat-follow-up.interrupt') },
            { value: 'queue', label: t('shell-settings.chat-follow-up.queue') },
          ]}
        />
        <SelectSetting
          part="settings.chat.sendShortcut"
          title={t('shell-settings.chat-send-shortcut')}
          description={t('shell-settings.chat-send-shortcut.hint')}
          source={text(fromRow(props.client, KEYS.sendShortcut), 'enter')}
          locked={locked}
          items={[
            { value: 'enter', label: t('shell-settings.chat-send-shortcut.enter') },
            { value: 'mod-enter-multiline', label: t('shell-settings.chat-send-shortcut.multiline', { mod }) },
            { value: 'mod-enter', label: t('shell-settings.chat-send-shortcut.modEnter', { mod }) },
          ]}
        />
      </SettingsGroup>
      <SettingsGroup part="settings.chat.display">
        <ChoiceSetting
          fieldset
          part="settings.chat.width"
          title={t('shell-settings.chat-width')}
          description={t('shell-settings.chat-width.hint')}
          source={text(fromRow(props.client, KEYS.chatWidth), 'comfortable')}
          locked={locked}
          items={[
            { value: 'comfortable', label: t('shell-settings.chat-width.comfortable') },
            { value: 'wide', label: t('shell-settings.chat-width.wide') },
            { value: 'full', label: t('shell-settings.chat-width.full') },
          ]}
        />
        <SelectSetting
          part="settings.chat.timeFormat"
          title={t('shell-settings.chat-time-format')}
          description={t('shell-settings.chat-time-format.hint')}
          source={text(fromRow(props.client, KEYS.timeFormat), 'locale')}
          locked={locked}
          items={[
            { value: 'locale', label: t('shell-settings.chat-time-format.locale') },
            { value: '12-hour', label: t('shell-settings.chat-time-format.12') },
            { value: '24-hour', label: t('shell-settings.chat-time-format.24') },
          ]}
        />
      </SettingsGroup>
    </>
  );
}
