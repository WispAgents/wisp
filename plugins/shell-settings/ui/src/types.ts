// The rows and keys the shell's own Settings panes read, until Rust generates them
// (packages/protocol/src/generated). Setting keys follow the manifests' `<prefix>.<v1 name>`
// (plan §6.2; /tmp/v2ref settings-api v1Name): the panes write each through `settings.set`.

import type { ExtensionId } from '@wisp/client';

/** The Settings overlay: `params.pane` names the open pane (none on a phone: the list); `filter`
 * reaches the pane as a prop (This Mac opens Worktrees on one filter). */
// in the plugin's namespace (a view id is its plugin's): the frame's Home, its sidebar and wisp.machine open it by it
export const VIEW_ID = 'shell-settings.sheet' as ExtensionId;
export interface SettingsParams {
  pane?: string;
  filter?: string;
  /** a setting's key: the search found it, and once the pane is built its row is scrolled to, lit and focused */
  focus?: string;
}

/** The registry keys behind the General, Appearance and Notifications rows (v1 config.defaults names after the dot). */
export const KEYS = {
  editor: 'settings.editor', // vscode | cursor | windsurf | zed | idea | none
  terminal: 'terminal.terminal', // a terminal.app id, or 'custom' (wisp.terminal's; audit A F5: was `terminal`, undeclared)
  terminalCommand: 'terminal.command',
  tui: 'terminal.terminalSessions', // 'on' | 'off', off by default
  blocks: 'runtime.blocks', // on unless 'off'
  worktree: 'worktrees.worktree', // on unless 'off'
  review: 'review.harness', // off | other | a harness id (wisp.review's setting; B53: was review.review, which nothing declares)
  risk: 'risk.harness', // off | other | a harness id (wisp.risk's merge risk, ADR-0143): off by default
  fixConflicts: 'ship.fixConflicts', // off unless 'on'
  readTimes: 'settings.readTimes', // on unless 'off'
  watch: 'runtime.watch', // on unless 'off'
  apps: 'runtime.apps', // on unless 'off'
  transcript: 'settings.transcript', // Chat mode: on unless 'off' (declared by wisp.settings)
  chatToolCalls: 'settings.chatToolCalls', // showAll | hideSuccessful | hideAll | zen
  chatThinking: 'settings.chatThinking', // bool, on by default
  followUp: 'runtime.followUp', // interrupt | queue
  sendShortcut: 'settings.sendShortcut', // enter | mod-enter-multiline | mod-enter
  chatWidth: 'settings.chatWidth', // comfortable | wide | full
  timeFormat: 'settings.timeFormat', // locale | 12-hour | 24-hour
  notify: 'settings.notify', // 'on' | 'off'
  acceptChannels: 'channels.acceptChannels', // off unless 'on'
  channelProject: 'channels.project', // a project id (its name), '' for the last used
  crewTint: 'crew.tint', // bool, on by default
} as const;

export const EDITORS = ['vscode', 'cursor', 'windsurf', 'zed', 'idea', 'none'] as const;

/** A terminal a task can open in (v1 config.terminals), on the config topic. */
export interface TerminalApp {
  id: string;
  label: string;
}
export const TERMINAL_KIND = 'terminal.app';

/** A harness, as wisp.accounts lists it (topic plugin:wisp.accounts:harnesses). */
export interface Harness {
  id: string;
  name: string;
  available: boolean;
  version?: string;
}
export const HARNESS_KIND = 'accounts.harness';
/** The harnesses "Review by another harness" may name (v1 REVIEW_HARNESSES). */
export const REVIEW_HARNESSES = ['claude', 'codex', 'antigravity', 'copilot', 'opencode', 'cursor', 'grok'] as const;
