// @wisp/shell's contract: how a plugin's UI plugs into the app (plan §6.6, ADR-0003, ADR-0007).
// The shell is an extension tree. Every slot is a typed *input* of a parent extension; plugins attach
// extensions to inputs, override an extension by id, disable one, or set its config. Built-in screens
// use exactly this API (no private API for first-party code, plan §6.0.3).

import type { Component, JSX } from 'solid-js';
import type { Client, PluginClient, Nav, Intent, Readable } from '@wisp/client';
import type { IconName, Toaster } from '@wisp/ui';
import type { Platform } from './platform/types.ts';

/** Inputs the shell's own extensions offer. v1 spell SLOTS map 1:1 (`feed.top` → `shell.list:top`). */
export const INPUTS = {
  // top bar
  'shell.topbar:brand': 'topbar.brand',
  'shell.topbar:projects': 'topbar.projects',
  'shell.topbar:machines': 'topbar.machines',
  'shell.topbar:getapp': 'topbar.getapp',
  'shell.topbar:automations': 'topbar.automations',
  'shell.topbar:search': 'topbar.search',
  'shell.topbar:account': 'topbar.account',
  'shell.topbar:settings': 'topbar.settings',
  'shell.topbar:view': 'topbar.view',
  'shell.account:menu': 'account.menu',
  // the task list (sidebar)
  'shell.list:top': 'sidebar.top',
  'shell.list:feedTop': 'feed.top',
  'shell.list:needs': 'section.needs',
  // added 2026-11-12 (additive): rows other plugins add at the end of Settings → General (v1 kept Updates there)
  'shell.settings:general': 'settings.general',
  // added 2026-11-18 (additive, ADR-0096): switches other plugins add to Settings → General's tasks group, after Watch
  // agents work and before Interactive answers (where v1 had Task ledger and Check compactions)
  'shell.settings:tasks': 'settings.tasks',
  // added 2026-11-12 (additive): what other plugins put in Settings → Machines under its first group (wisp.grid-client's
  // Reach this computer from anywhere, v1 .mc-grid)
  'shell.settings:machines': 'machines.reach',
  // added 2026-11-12 (additive): rows other plugins add to Settings → Usage's first group (wisp.crew's Crew track
  // record), and groups at its end (wisp.machine's Machine load), where v1 had them
  'shell.settings:usage': 'usage.links',
  'shell.settings:usageEnd': 'usage.end',
  // added 2026-11-12 (additive): notes on Home under what needs you (wisp.budget's Spare allowance and window
  // changes, v1 .sh-spare); its extensions get `pickAgent`
  'shell.home:cards': 'home.cards',
  // added 2026-11-14 (additive): under Home's prompt box, usage at a glance (wisp.budget's plan windows, v1 #790 .sh-usage)
  'shell.home:usage': 'home.usage',
  // added 2026-11-16 (additive): at the end of New task's Done when line, who judges the goal (wisp.goal's Judges,
  // v1 #judgesBtn)
  'shell.home:goalLine': 'launcher.goalJudges',
  // added 2026-11-17 (additive): Home's last line, wisp.telemetry's "Hit a bug…" while feedback can be sent (v1 .sh-fb)
  'shell.home:feedback': 'split.feedback',
  // added 2026-11-13 (additive): in the harness · model picker, a signed-out harness's sign-in, right there (wisp.accounts'
  // link-and-code steps, v1 runner-picker.js paintSignIn); its extension gets the harness as `id`
  'shell.runner:signin': 'runner.signin',
  // added 2026-11-13 (additive): what other plugins put in Settings → Cloud under the account (wisp.machines' cloud
  // computers, v1 .cg-list: Open, Wake / Put to sleep, its agents)
  'shell.settings:cloud': 'cloud.computers',
  // added 2026-11-13 (additive): a machine's coding agents, signed in to from here (wisp.accounts, v1 cloud.js agents: the
  // login runs on that computer and stays there); its extension gets the machine's peer as `id`
  'shell.machine:agents': 'machine.agents',
  // added 2026-11-12 (additive): what goes between Needs you and Working (the crew's section on the phone, v1 #secCrew);
  // in the phone apps (data-platform) it goes under the Tasks top bar instead, over the project chips (the crew's
  // stories, ADR-0065 step 13b), and the list leaves it out
  'shell.list:crew': 'section.crew',
  // added 2026-11-12 (additive): what a crew face's card beside the rail says (v1 .cl-card: who it is, the tasks it's on);
  // its extension gets the member's `id`. Without one, a face has no card.
  'shell.rail:card': 'crew.card',
  'shell.list:working': 'section.working',
  'shell.list:scheduled': 'section.scheduled',
  'shell.list:scheduledRow': 'scheduled.row',
  'shell.list:drafts': 'section.drafts',
  'shell.row:head': 'card.head',
  'shell.row:hover': 'card.hover',
  'shell.row:menu': 'card.menu',
  'shell.row:top': 'card.top',
  'shell.row:bottom': 'card.bottom',
  // the open task
  'shell.task:head': 'split.head',
  'shell.task:details': 'split.details',
  'shell.task:panes': 'split.panes',
  'shell.task:upcoming': 'split.upcoming',
  'shell.task:band': 'goal.band',
  // added 2026-11-10 (additive): over the reply box, what waits on you (the agent's question)
  'shell.task:foot': 'task.foot',
  // added 2026-11-15 (additive): inside the reply box, over the text, within its border (v1 .c-queued): the queued
  // follow-ups (shell-composer, order 10), then wisp.later's set for a time (order 20); the box grows upward with what's here
  'shell.task:composer-top': 'composer.top',
  // added 2026-11-10 (additive): under each turn (its key as the id), what plugins say of it (why it stopped, held, the receipt)
  'shell.turn:foot': 'turn.foot',
  // added 2026-11-17 (additive, ADR-0095): in a turn's trail, after its token count, one line with it (its key as the id):
  // the receipt's cache and plan share (v1 receipt.js, receiptHtml inside .trail-bar)
  'shell.turn:trail': 'turn.trail',
  'shell.side:head': 'panel.head',
  'shell.term:head': 'term.head',
  // composer and new task
  'shell.composer:bar': 'composer.bar',
  'shell.composer:chips': 'composer.chips',
  'shell.launcher:bar': 'launcher.bar',
  'shell.launcher:chips': 'launcher.chips',
  // other screens
  'shell.crew:page': 'crew.page',
  'shell.crew:rail': 'crew.rail',
  'shell.org:head': 'org.head',
  'shell.plan:head': 'plan.head',
  'shell.plan:page': 'plan.page',
  'shell.plan:prompt': 'plan.prompt',
  'shell.plan:details': 'plan.details',
  'shell.plan:actions': 'plan.actions',
  'shell.stuff:head': 'stuff.head',
  'shell.machine:head': 'machine.head',
  'shell.getapp:card': 'getapp.card',
  'shell.palette:tags': 'palette.tags',
  'shell.switcher:row': 'switcher.row',
  // whole areas a plugin may override by id (Backstage overrides)
  'shell.app:main': null,
  'shell.app:side': null,
  'shell.app:overlay': null,
  'shell.settings:pane': null,
  'shell.block:renderer': null,
  // added 2026-11-08 (additive): the areas the frame renders, each one extension (the last
  // registered, or an override) — shell-list fills the list, shell-home/shell-task the screens
  'shell.sidebar:list': null,
  'shell.screen:home': null,
  'shell.screen:task': null,
  'shell.screen:crew': null,
  'shell.side:changes': null,
  'shell.side:shell': null,
  'shell.side:pr': null,
  // added 2026-11-12 (additive): the side panel's Preview (v1 run.js), the task's app run from its folder
  'shell.side:preview': null,
} as const satisfies Record<string, string | null>;

export type InputId = keyof typeof INPUTS;

/** A send a check is asked about (`ShellContext.sendHolds`). */
export interface SendHoldAsk {
  readonly peer: string;
  readonly agent: string;
  readonly account?: string;
  /** the task a follow-up goes to (missing: a new task) */
  readonly session?: string;
}

/** Why a send waits: the row's sentence and when it may go without asking (the window's reset). */
export interface SendHold {
  readonly reason: string;
  readonly say: string;
  /** ms since the epoch */
  readonly until?: number;
}

export interface SendHolds {
  provide(reason: string, check: (a: SendHoldAsk) => Promise<SendHold | undefined>): () => void;
  /** every check, in turn: the first hold, or undefined (a failing check counts as no hold: the server asks again) */
  check(a: SendHoldAsk): Promise<SendHold | undefined>;
  /** the check for one refusal reason, after the server gave it */
  answers(reason: string | undefined): ((a: SendHoldAsk) => Promise<SendHold | undefined>) | undefined;
}

/** What an extension's component receives. `session` is set inside task-scoped inputs (row, task, composer). */
export interface ExtensionProps<C = Record<string, unknown>> {
  client: PluginClient;
  config: C;
  session?: () => { id: string; agent: string; [k: string]: unknown } | undefined;
  /** added: inside a bar input, the form the Toolbar gives the item ('full' = icon and label) */
  form?: 'full' | 'icon';
  /** added: on a screen or side input, the entity it shows (a task's key, a crew member's id) */
  id?: string;
  /** added 2026-11-12: on Home's inputs, puts a harness in Home's prompt box (Spare allowance's Use) */
  pickAgent?: (agent: string) => void;
  /** added 2026-11-18 (additive): on Home's inputs, the harness in Home's prompt box now ('' before one is picked) */
  agent?: () => string;
}

/** A task's menu in sections, from the task itself to Delete (v1 #786, design/ux.md → Task menus): what the bar
 *  does (Open, Rerun…), the task itself (Rename, Fork…, Share…), what's waiting on it (Ship it, Merge PR), where to
 *  look (View ›, Goal ›, crew), what to keep (Copy ›, Save as recipe…, Files ›), where it sits (Mark done, Save for
 *  later, Archive, then the rest). */
export type TaskMenuSection = 'open' | 'task' | 'waiting' | 'look' | 'keep' | 'place';
export type TaskMenuSub = 'view' | 'goal' | 'copy' | 'files';

export interface ExtensionDef<C = Record<string, unknown>> {
  id: string; // owner-prefixed: 'claude.model-picker'
  attachTo: { id: InputId | (string & {}); input?: string };
  /** lazy(() => import('./X.tsx')): a plugin's code loads on first show */
  component: Component<ExtensionProps<C>>;
  /** collapsed form in a bar (design/layout.md §2): required for anything attached to a bar input */
  bar?: {
    icon: string;
    label: string;
    priority: number | 'pin';
    /** added: what choosing it from the bar's More menu does (default: nothing) */
    select?: (p: ExtensionProps<C>) => void;
    /** added 2026-11-12 (additive): its key, shown beside it in a menu ('⇧R') */
    keys?: string;
    /** added 2026-11-12 (additive): its words for this task, when they depend on it ("Push to PR #12", "Stop keeping
     *  worktree"); `label` otherwise */
    text?: (p: ExtensionProps<C>) => string;
    /** added 2026-11-12 (additive): in a task's row menu, 'end' puts it after the list's own Save for later (v1's Keep
     *  worktree); before the list's triage items otherwise */
    place?: 'end';
    /** added 2026-11-12 (additive): in the row's menu only, not the open task's More (its bar has it: Ship it) */
    rowOnly?: boolean;
    /** added 2026-11-12 (additive, v1 #786): the section of a task's menu it sits in (default 'place', after Archive) */
    section?: TaskMenuSection;
    /** added 2026-11-12 (additive, v1 #786): the submenu it folds into; it sets the section (View and Goal: 'look';
     *  Copy and Files: 'keep') */
    sub?: TaskMenuSub;
  };
  config?: { parse(v: unknown): C; default: C };
  if?: (p: ExtensionProps<C>) => boolean;
  /** an override replaces the extension with this id */
  overrides?: string;
  order?: number;
}

/** A view kind a plugin registers for navigation (`main` or `side`), with typed params (plan §4.6.4). */
export interface ViewDef<P = unknown> {
  kind: string; // 'crew.member'
  slot: 'main' | 'side' | 'overlay';
  parse(params: unknown): P;
  component: Component<{ client: PluginClient; params: P }>;
  title?(params: P): string;
  /** added: an overlay view's presentation (default 'dialog'; a sheet on phones either way) */
  overlay?: 'viewer' | 'dialog' | 'sheet';
  /** added 2026-11-10 (additive): the view draws its own head and close (Settings); the dialog adds none */
  bare?: boolean;
  /** added 2026-11-12 (additive): 'top' draws the overlay near the top over a light scrim (v1's ⌘K palette) */
  place?: 'top';
  /** added 2026-11-12 (additive): another name for the dialog it opens in (Settings: v1's `settings`, the .modal.sx box) */
  part?: string;
  /** added 2026-11-12 (additive): the dialog's size ('v1': v1's .modal, 640 wide) */
  size?: 'sm' | 'md' | 'lg' | 'v1';
}

/** A Settings pane. */
export interface SettingsPaneDef {
  id: string;
  group: 'general' | 'agents' | 'usage' | 'integrations' | 'computers' | 'advanced' | (string & {});
  title: string; // i18n key
  icon: string;
  component: Component<{ client: PluginClient }>;
  order?: number;
  /** shown only while this says so (read reactively: a setting, a switch); absent: always. Additive (2026-11-17). */
  if?: (client: PluginClient) => boolean;
}

/** An answer block renderer (the `runtime.block` point's UI half). */
export interface BlockRendererDef {
  type: string;
  component: Component<{ block: Record<string, unknown>; client: PluginClient; sessionId: string }>;
}

export interface PluginUiDef {
  id: string; // the plugin id: 'wisp.crew'
  /** built with defineExtension(), which erases C after checking it */
  extensions?: ExtensionDef[];
  /** built with defineView(), which erases P after checking it */
  views?: ViewDef[];
  settings?: SettingsPaneDef[];
  blocks?: BlockRendererDef[];
  /** commands this UI declares (defineCommand from @wisp/client) */
  commands?: unknown[];
  /** i18n catalogue: key → English */
  catalogue?: Record<string, string>;
  /** runs once when the plugin's UI activates; everything registered belongs to its scope */
  setup?(client: PluginClient): void;
  /** added: how this plugin configures the tree (a spell's PART_PROPS, moves, removals) */
  tree?: TreeConfig;
  /** added: what it adds to the command palette (⌘K) */
  palette?: PaletteSourceDef[];
  /** added: its keyboard shortcuts (only keys the shell doesn't use itself, see BUILT_IN_KEYS) */
  shortcuts?: ShortcutDef[];
  /** added 2026-11-11 (additive): what a prompt box's @ and / menus offer (v1 composer.js) */
  prompt?: PromptSourceDef[];
  /** added 2026-11-17 (additive, TRY-19): places New task's Run on offers besides your computers (wisp.sandbox: a fresh
   *  cloud box); picked, the prompt goes to the place's `launch` instead of a computer */
  launchTargets?: LaunchTargetDef[];
  /** added 2026-11-08 (additive): the coding agents it runs, as the screens name and mark them (a
   * harness plugin's UI: plugins/claude/ui). The shell knows none of them itself (ADR-0003). */
  harnesses?: HarnessUiDef[];
  /** added 2026-11-12 (additive): tabs it adds to the panel beside a task (v1 panel.js: My Stuff's panes pinned to tasks) */
  sideTabs?: SideTabDef[];
  /** added 2026-11-14 (additive, ADR-0064): where a `preview` block's page loads when it can't as written (wisp.grid-client:
   *  a box's `localhost:<port>` at the grid's preview address). The first that answers wins; none: the URL as written. */
  previewAddresses?: PreviewAddressDef[];
  /** added 2026-11-14 (additive, ADR-0067): what of it only a press reaches (a row's menu items, the palette's, its
   *  shortcuts and prompt sources, the tabs beside a task), loaded with import() as the first screen draws and
   *  registered into this same plugin (its scope, its client), so the first page doesn't carry its code (ADR-0011).
   *  Views, Settings panes, harnesses, the catalogue, setup and whatever the first screen draws stay in the
   *  definition: a link or the last place must find its view or pane at start. */
  later?: () => Promise<{ readonly default: PluginUiLater }>;
  /** added 2026-11-16 (additive, ADR-0091): what may make a send wait for you (Ask near the limit), asked by the prompt
   *  boxes before they send and when the server refuses one with `reason` (`ShellContext.sendHolds`) */
  sendHolds?: SendHoldDef[];
  /** added 2026-11-18 (additive, docs/ux/2026-11-18-grid.md #9): links it puts in the account menu, shown while `href`
   *  gives one (wisp.grid-client: wisp Grid, from a cloud wisp back to its grid's Home) */
  accountLinks?: AccountLinkDef[];
  /** added 2026-11-18 (additive, docs/ux/2026-11-18-grid.md #9): set-up tips it puts on Home, shown while `offered`
   *  says so (wisp.grid-client: Reach this wisp from anywhere, never on a grid box). The shell knows none of them. */
  homeTips?: HomeTipDef[];
}

/** One link in the account menu, from a plugin: opened as a link (`href`, read with the plugin's own client). */
export interface AccountLinkDef {
  id: string;
  /** the plugin's own words for it (its `t`) */
  label(): string;
  hint?(): string;
  icon: IconName;
  /** where it goes now (may read signals); undefined: not offered */
  href(client: PluginClient): string | undefined;
}

/** One set-up tip on Home, from a plugin. */
export interface HomeTipDef {
  id: string;
  icon: IconName;
  title(): string;
  body(): string;
  cta(): string;
  /** whether Home offers it now (may read signals); unset: always */
  offered?(client: PluginClient): boolean;
  /** its button; `settings(pane)` opens a Settings pane as the shell does */
  open(ctx: { client: PluginClient; settings: (pane: string) => void }): void;
}

/** One check a prompt box asks before a send (and after the server refuses one with `reason`). */
export interface SendHoldDef {
  /** the refusal reason its server-side guard gives (`budget_hold`) */
  reason: string;
  check(ctx: { client: PluginClient }, a: SendHoldAsk): Promise<SendHold | undefined>;
}
/** What a plugin's UI registers once it's loaded (PluginUiDef.later): the parts the first screen never calls. */
export type PluginUiLater = Pick<PluginUiDef, 'extensions' | 'palette' | 'shortcuts' | 'prompt' | 'sideTabs'>;

/** One way to show a `preview` block's page from where this window is (ADR-0064). */
export interface PreviewAddressDef {
  id: string;
  /** called as an answer of the task `task` (an entity key) sets up; the function it gives reads reactively and
   *  says where `url` loads from here, or undefined when it isn't this plugin's to say */
  map(ctx: { client: PluginClient; task: string }): (url: string) => string | undefined;
}

/** Tabs a plugin adds to the panel beside a task, after the panel's own (Changes, PR, Shell, Preview). */
export interface SideTabDef {
  id: string;
  /** among plugins' tabs, lower first */
  order?: number;
  /** its tabs beside the task `task` (an entity key), read reactively: a pin added or removed shows at once */
  tabs(ctx: { client: PluginClient; task: string }): readonly SideTabItem[];
  /** what its tab `value` shows, in the panel's body, edge to edge */
  component: Component<{ client: PluginClient; task: string; value: string }>;
}

export interface SideTabItem {
  value: string;
  label: string;
  /** its tooltip (v1: "Name: pinned from My Stuff") */
  title?: string;
  /** added 2026-11-18 (additive, ADR-0079): its icon (a kit icon name); default 'app' (an app pinned from My Stuff) */
  icon?: string;
  /** added 2026-11-18 (additive, UX #5): nothing new in it for now: it waits behind the strip's + until opened */
  quiet?: boolean;
}

/** A coding agent as the screens show it: its name, its mark, and whether it's the one a new task
 * runs on when nothing else is picked. `id` is the agent id rows carry (`session.agent`). */
export interface HarnessUiDef {
  id: string;
  name: string;
  /** an image URL: the plugin's own file (`new URL('./logo.svg', import.meta.url).href`) or a data: URL. A one-colour
   *  mark ends in `#mono` (updated 2026-11-18, additive): it is drawn in the app theme's ink, whatever the OS's scheme */
  logo?: string;
  /** the default for a new task (the first registered default wins) */
  default?: boolean;
  /** the order in pickers */
  order?: number;
  /** added 2026-11-14 (additive): the run settings Try it two ways may change on it (`subagentModel`, `skills`), as
   *  its backend's `HarnessInfo.variants` says (ADR-0056); none: the launcher offers no What's different */
  variants?: readonly string[];
}

export interface ShellApi {
  client: Client;
  nav: () => Nav;
  register(def: PluginUiDef): void;
  render(root: HTMLElement): () => void;
  // added 2026-11-08 (additive)
  /** what the screens read through useShell() */
  readonly context: ShellContext;
  /** register returns nothing (frozen); this switches a plugin's UI off again: its scope ends */
  unregister(id: string): void;
  dispose(): void;
}

// ---------- added 2026-11-08 (additive): tree config, palette, shortcuts, the shell context ----------

/** A plugin's say over other extensions. Later plugins win over earlier ones, key by key. */
export interface TreeConfig {
  /** extensions (by id) that no longer show */
  disable?: string[];
  /** an extension's config, checked by its own `config.parse` (a bad value keeps the default) */
  config?: Record<string, unknown>;
  /** moves an extension to another input */
  attach?: Record<string, ExtensionDef['attachTo']>;
  /** an extension's place among its siblings (lower first) */
  order?: Record<string, number>;
}

/** An extension as the shell resolved it: where it sits, its parsed config, its place. */
export interface ResolvedExtension {
  readonly id: string;
  readonly plugin: string;
  readonly def: ExtensionDef;
  readonly config: Record<string, unknown>;
  readonly order: number;
}

/** One row of the command palette (v1 web/js/palette.js items). */
export interface PaletteItem {
  id: string;
  /** an i18n key or already-translated text: the group heading ("This task", "Go to") */
  group: string;
  label: string;
  sub?: string;
  icon?: string;
  /** the shortcut hint shown on the row ('E', '⇧R') */
  kbd?: string;
  /** a project's colour: the row leads with its dot instead of an icon */
  color?: string;
  /** extra words it matches */
  keywords?: string;
  /** topic words: "#auth" in the query keeps only items tagged auth */
  tags?: readonly string[];
  /** ms: when it was last touched; recent items rank higher */
  at?: number;
  /** a fixed rank inside its group (lower first) when the query is empty */
  rank?: number;
  /** choosing it types this into the input instead of running anything */
  fill?: string;
  run?(): void;
  /** added 2026-11-14 (additive): a second act at the row's end that leaves the palette open (a project's + / −:
   * add it to the pick, ⌘-Enter); `on` shows the check and the − */
  toggle?: { readonly on: boolean; readonly label: string; run(): void };
}

export interface PaletteContext {
  client: PluginClient;
  nav: Nav;
  /** the task the keyboard is on (or the open one) */
  focused: string | undefined;
  /** opened on some sources alone (`only`: ⌘P, Changes' Open file…): a source that only serves such a palette
   *  answers nothing in ⌘K. Additive (2026-11-14). */
  narrowed?: boolean;
}

export interface PaletteSourceDef {
  id: string;
  /** with an empty query, sources show in this order (lower first) */
  order?: number;
  items(ctx: PaletteContext): readonly PaletteItem[];
  /** more items for a query from the peer (past work by its words); they follow the rest */
  deep?(query: string, ctx: PaletteContext, signal: AbortSignal): Promise<readonly PaletteItem[]>;
}

/** One row of a prompt box's @ or / menu. */
export interface PromptItem {
  value: string;
  label: string;
  detail?: string;
  hint?: string;
  icon?: string;
  /** what replaces the trigger and what was typed after it (`/goal `, `@Fix the login bug `) */
  insert: string;
  /** a mention: sent as `@[title](scheme:id)` (v1 mention-text.js), shown as `@title` */
  mention?: { scheme: 'wisp' | 'crew' | 'stuff'; id: string; title: string };
  /** a second way to take it (an Open button on the row, ⌘↵): a file opens in the task's file view. Additive (2026-11-11). */
  open?: () => void;
}

/** New task's picks, for a prompt source that takes what's sent there (`/sandbox …`). Additive (2026-11-11). */
export interface LaunchPick {
  readonly cwd?: string;
  readonly agent?: string;
  readonly model?: string;
  readonly effort?: string;
  readonly mode?: string;
  readonly goal?: unknown;
  /** a start time picked on New task's clock (ms): a source may schedule instead of starting it. Additive (2026-11-12). */
  readonly at?: number;
}

/** A place New task can run a prompt besides your computers (Run on), from a plugin. Additive (2026-11-17, TRY-19). */
export interface LaunchTargetDef {
  id: string;
  /** the plugin's own words for it (its `t`) */
  label(): string;
  icon: IconName;
  /** among targets, lower first */
  order?: number;
  /** whether Run on offers it now (may read signals); unset: always */
  offered?(client: PluginClient): boolean;
  /** takes the prompt with New task's picks; New task clears its box once this returns */
  launch(prompt: string, ctx: { client: PluginClient; launch: LaunchPick }): void;
}

/** What a prompt box's @ or / menu offers, from a plugin (shell-composer: tasks, crew, My Stuff, the commands). */
export interface PromptSourceDef {
  id: string;
  trigger: '@' | '/';
  /** the boxes it serves: a task's reply box, New task's (both when unset) */
  where?: readonly ('reply' | 'home')[];
  /** among sources for one trigger, lower first */
  order?: number;
  /** `project`: New task's picked project's folder (additive, 2026-11-11) */
  items(query: string, ctx: { client: PluginClient; task?: string; project?: string }): readonly PromptItem[];
  /**
   * A reply this source takes instead of the task's agent (crew: a reply that starts with @a member is for them). Gets
   * the stored text (mentions encoded); true: it was taken (and sent), the box clears and nothing goes to the agent.
   * Additive (2026-11-11). `restore` (additive, 2026-11-12, reply box): puts the text back in the box when what it
   * started was refused, unless something new was typed meanwhile. `say` (additive, 2026-11-19, New task's box): a
   * short notice of what it did where the box can't show it (Start later: "Starts 03:56", the plan in Scheduled).
   */
  send?(text: string, ctx: { client: PluginClient; task: string; launch?: LaunchPick; restore?: (text: string) => void; say?: (text: string) => void }): boolean;
  /**
   * What goes with the task's next follow-up, at its top (comments on lines of its Changes, v1 comments.js): the
   * follow-up as it will be sent, and `taken()` once it went (the tray empties), `back()` if it was refused.
   * A reply with nothing typed still goes when something waits. Additive (2026-11-12).
   */
  pending?(text: string, ctx: { client: PluginClient; task: string }): { readonly text: string; taken(): void; back(): void } | undefined;
  /**
   * How many things wait to go with the task's next follow-up (comments not sent yet): Ship and Merge say so before
   * they go (v1 #F-page-11). Read when asked, not watched. Additive (2026-11-12).
   */
  waiting?(ctx: { client: PluginClient; task: string }): number;
  /**
   * Comments on the task's answers (v1 comments.js, F-page-10): `add` keeps one for the next follow-up (`pending` then
   * takes it with the rest), `marks` are those waiting on one turn's answer, numbered as the tray numbers them, read
   * reactively so the answer marks them. Additive (2026-11-14).
   */
  answer?: {
    add(c: AnswerNote, ctx: { client: PluginClient; task: string }): void;
    marks(turn: string, ctx: { client: PluginClient; task: string }): readonly { n: number; i: number; quote?: string; note: string }[];
  };
}

/** A comment on an answer: its turn, the block (by index) and what it is ("Diff · server/auth.js"), the words picked. */
export interface AnswerNote {
  /** set by the source that keeps it (a stable key for its row) */
  id?: string;
  turn: string;
  i: number;
  label: string;
  kind: 'text' | 'block';
  quote?: string;
  note: string;
  /** an earlier answer's prompt: the agent is told which answer it was on (v1 `of`) */
  of?: string;
}

/** 'e', 'shift+r', 'mod+k' (⌘ on a Mac, Ctrl elsewhere), 'ctrl+`', 'escape', '?' */
export type KeyCombo = string;

export interface ShortcutContext {
  client: PluginClient;
  nav: Nav;
  focused: string | undefined;
  event: KeyboardEvent;
}

export interface ShortcutDef {
  id: string;
  keys: KeyCombo;
  /** i18n key: the row in the shortcuts sheet (?) */
  label: string;
  group?: 'general' | 'task' | 'list' | (string & {});
  /** 'idle' (default): not while typing or with an overlay up · 'always': even mid-sentence ·
   * 'task': idle, and a task is focused */
  when?: 'idle' | 'always' | 'task';
  /** false: not handled, the key goes on */
  run(ctx: ShortcutContext): boolean | void;
}

export type ThemePref = 'system' | 'light' | 'dark';

/** A prompt box another plugin may fill (ShellContext.prompts). */
export interface PromptBox {
  text(): string;
  set(text: string): void;
  focus(): void;
  /** added 2026-11-25 (additive): files as the box's attachments (chips; they go up with Send), as a pick would */
  attach?(files: readonly File[]): void;
}

/** What every screen reads through useShell(): the shell's own state and services. */
export interface ShellContext {
  readonly client: Client;
  readonly nav: () => Nav;
  navigate(intent: Intent): void;
  /** t('key', vars): every user-facing string */
  t(key: string, vars?: Readonly<Record<string, string | number>>): string;
  readonly toast: Toaster;
  readonly platform: Platform;
  readonly theme: { pref(): ThemePref; resolved(): 'light' | 'dark'; set(pref: ThemePref): Promise<void> };
  /** the task the keyboard is on (j/k, a row's focus): UI state the list and the keys share */
  readonly selection: { get(): string | undefined; set(key: string | undefined): void };
  /** the list beside the task (split view): `[` folds it */
  readonly sidebar: { open(): boolean; toggle(): void; width(): number; setWidth(px: number): void };
  /** added 2026-11-11 (additive): the project the list shows, set by the top bar's chips ('all', or a project's id);
   * a list in `shell.sidebar:list` follows it. Since 2026-11-14 several picked projects are their ids joined by
   * commas, in the order added (frame/project-pick.ts `parsePick`) */
  readonly projectFilter?: { readonly value: Readable<string>; set(id: string): void };
  /** added 2026-11-21 (additive): whether the phone apps' Tasks destination is offered yet (the phone app: a computer
   * you opened has connected); false draws Tasks dimmed and a press does nothing. Absent: always */
  readonly tasksReady?: Readable<boolean>;
  /** added 2026-11-15 (additive, ADR-0065 step 13): the phone apps' computer filter, the Tasks bar's title ('all', or a
   * peer id: frame/computer-filter.ts); a list in `shell.sidebar:list` shows only that computer's rows. The browser
   * leaves it at 'all' (no control sets it there) */
  readonly computerFilter?: { readonly value: Readable<string>; set(peer: string): void };
  /** added 2026-11-15 (additive, ADR-0065 step 13): the task keys in the order the list shows them now (its filters,
   * folded sections, sort), which the list publishes while it is mounted (the last order stays after it goes). A phone's
   * task page steps through it (Next task / Previous task, the head's swipe: frame/task-step.ts) */
  readonly listOrder?: { readonly keys: Readable<readonly string[]>; publish(keys: readonly string[]): void };
  /** added 2026-11-15 (additive, ADR-0065 step 13): the phone list's selection mode (frame/list-select.ts): the bar
   * becomes the selection bar, a row's tap toggles it. UI state the list's rows and the shell's bar share */
  readonly listSelect?: {
    readonly on: Readable<boolean>;
    readonly keys: Readable<readonly string[]>;
    /** enter it, with `key` picked (a row's Select) or none (the bar's Select) */
    start(key?: string): void;
    toggle(key: string): void;
    all(): void;
    exit(): void;
  };
  /** named things a shortcut can focus ('launcher' for ⌘L, 'reply' for /): the owner registers */
  readonly focusTargets: { provide(name: string, focus: () => void): () => void; focus(name: string): boolean };
  /** added 2026-11-11 (additive): the prompt boxes ('launcher' for New task) a plugin may fill, as v1's Start from an
   * issue fills New task's prompt; the owner registers its box while it shows */
  readonly prompts?: {
    provide(name: string, box: PromptBox): () => void;
    get(name: string): PromptBox | undefined;
    /** added 2026-11-25 (additive): files for the box `name`, handed to it now or, while it isn't shown (a share
     *  opens New task, which mounts after), the moment it is */
    attach?(name: string, files: readonly File[]): void;
  };
  /** added 2026-11-16 (additive, ADR-0091): what may make a send wait for you (Ask near the limit, v1 hold.js). A plugin
   * provides a check under the refusal reason its server-side guard gives (`budget_hold`); the prompt boxes ask every
   * check before they send (Send shows pending meanwhile) and show the first hold as a row in the box, and a send the
   * server refuses with that reason gets the same row instead of a toast. */
  readonly sendHolds?: SendHolds;
  /** added 2026-11-12 (additive): the list's task card, lent to other screens: the phone's Crew section nests a member's
   * jobs as list cards (v1 crew.js placeCards). The list registers its renderer while it shows; `render` is undefined
   * until then (and with no list plugin), so a borrower shows nothing rather than a card of its own. */
  readonly taskCards?: { provide(render: (key: string) => JSX.Element): () => void; readonly render: Readable<((key: string) => JSX.Element) | undefined> };
  readonly palette: { open(query?: string): void };
  /** `run` once the first screen is drawn (ADR-0105): deferred work (a warm-up, a preload) never competes with it, and
   *  the budget gate's run never sees it. Additive (2026-11-18). */
  readonly afterFirstScreen?: (run: () => void) => void;
  /** What a harness's Default runs, by name, on `peer` in `cwd` (runtime.defaultModel: the folder's config, else the
   *  plan's; else the computer's harnesses topic), one answer the whole UI shares, so a task on the default model reads
   *  the same everywhere (the chip, the facts, the list, Compare). Undefined while asked or when it can't tell. Reactive.
   *  Additive (2026-11-18). */
  readonly defaultModel?: (agent: string, at: { readonly peer: string; readonly cwd?: string | undefined }) => string | undefined;
  /** the extensions on one input, resolved (overrides, disable, attach, config, order) */
  extensions(input: string): () => readonly ResolvedExtension[];
  settingsPanes(): () => readonly SettingsPaneDef[];
  /** added 2026-11-08 (additive): the coding agents the harness plugins registered, in picker order */
  harnesses(): () => readonly HarnessUiDef[];
  /** added 2026-11-11 (additive): every plugin's prompt sources (the @ and / menus) */
  promptSources(): () => readonly PromptSourceDef[];
  /** added 2026-11-17 (additive, TRY-19): every plugin's Run on places, in order */
  launchTargets(): () => readonly LaunchTargetDef[];
  /** added 2026-11-14 (additive, ADR-0067): plugins' later parts still on their way (a load that failed is tried
   *  again): a promise that settles when they have, or undefined when none is. A prompt box's send waits on it, so
   *  `/goal`, `@member` or `tomorrow 9am` sent as the page loads still reaches its prompt source. */
  laterReady?(): Promise<void> | undefined;
  /** added 2026-11-12 (additive): every plugin's tabs beside a task, each with its plugin's client */
  sideTabs?(): () => readonly (SideTabDef & { readonly plugin: string; readonly client: PluginClient })[];
  /** added 2026-11-14 (additive, ADR-0064): every plugin's preview addresses, each with its plugin's client */
  previewAddresses?(): () => readonly (PreviewAddressDef & { readonly plugin: string; readonly client: PluginClient })[];
  /** added 2026-11-18 (additive, docs/ux/2026-11-18-grid.md #9): every plugin's account-menu links and Home set-up
   *  tips, each with its plugin's client */
  accountLinks?(): () => readonly (AccountLinkDef & { readonly plugin: string; readonly client: PluginClient })[];
  homeTips?(): () => readonly (HomeTipDef & { readonly plugin: string; readonly client: PluginClient })[];
  block(type: string): BlockRendererDef | undefined;
  view(kind: string): ViewDef | undefined;
}

export type { JSX };
