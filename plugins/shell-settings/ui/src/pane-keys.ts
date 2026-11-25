// The settings each of the shell's own panes has a row for (SettingsPaneDef.keys): Settings' search opens a setting on
// the pane that shows it and lands on its row (channels.project is Notifications', not its plugin's Phone pane). Plain,
// so the search's test reads every pane's list without the panes.
import { KEYS } from './types.ts';

export const PANE_KEYS = {
  general: [
    KEYS.editor,
    KEYS.terminal,
    KEYS.terminalCommand,
    KEYS.tui,
    KEYS.blocks,
    KEYS.worktree,
    KEYS.review,
    KEYS.fixConflicts,
    KEYS.readTimes,
    KEYS.watch,
    KEYS.apps,
    KEYS.crewTint,
    // wisp.threads' row in General's tasks group (shell.settings:tasks)
    'threads.compactAudit',
  ],
  chat: [KEYS.transcript, KEYS.chatToolCalls, KEYS.chatThinking, KEYS.followUp, KEYS.sendShortcut, KEYS.chatWidth, KEYS.timeFormat],
  notifications: [KEYS.notify, KEYS.acceptChannels, KEYS.channelProject],
  keys: [],
} as const satisfies Record<string, readonly string[]>;
