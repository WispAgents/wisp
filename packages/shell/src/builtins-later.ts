// The shell's own part only a press reaches (ADR-0067: PluginUiDef.later), loaded as the first screen draws: the
// palette's sources (tasks, crew, Actions, projects) and the keys the shell answers itself (v1 keys.js). The
// first page doesn't carry their code (ADR-0011); builtins.tsx keeps the views and panes a link must find at start.
import { batch } from 'solid-js';
import type { ExtensionId, Json } from '@wisp/client';
import type { PaletteItem, PluginUiLater, ShellContext, ShortcutDef } from './contract.ts';
import { SESSION, type SessionRow } from './rows.ts';
import { archive, forLater, markRead, setState } from './frame/commands.ts';
import { pastWork, taskItems } from './palette/tasks.ts';
import { PENDING_KEY, TASK_KEY, type TaskKey } from './frame/task-keys.ts';
import { afterNextPaint } from './frame/task-paint.ts';
import { openFeedback } from './frame/feedback.ts';
import { PLACE_SOURCE, PROJECT_ACTIONS, PROJECT_SOURCE, PROJECT_TAIL, placeItems, projectItems, projectShortcuts, projectTail } from './frame/project-switch.ts';
import { doneRefused, flatRow, isSaved, undoneState } from './frame/rows-view.ts';
import { markStepped } from './frame/stepped.ts';

const overlay = (id: string): { t: 'openOverlay'; overlay: { kind: 'dialog'; id: ExtensionId; params: null } } => ({
  t: 'openOverlay',
  overlay: { kind: 'dialog', id: id as ExtensionId, params: null },
});

// v1 features/commands.js paletteItems(): the tasks, the crew, then Actions (New task first, the themes you're not on
// near the end), the projects last; an empty query shows each group's first six.

/** Actions, after New task: adding and managing projects (v1's next rows; the ⌘P switcher lists them too). */
function actionHead(ctx: ShellContext): PaletteItem[] {
  const g = 'shell.palette.group.actions';
  const projects = (): void => ctx.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.projects' as never, params: null } });
  return [
    { id: 'project:add', group: g, label: ctx.t('shell.cmd.addProject'), icon: 'folder', keywords: 'new project folder repo directory', run: projects },
    { id: 'project:manage', group: g, label: ctx.t('shell.side.manage'), icon: 'folder', keywords: 'order rename colour color remove', run: projects },
  ];
}

/** Actions, its tail: the list, the keys, the two themes you're not on, diagnostics, home. */
function appItems(ctx: ShellContext): PaletteItem[] {
  const g = 'shell.palette.group.actions';
  const theme = (pref: 'light' | 'dark' | 'system', label: string, icon: string): PaletteItem => ({
    id: `shell.theme.${pref}`,
    group: g,
    label: ctx.t(label),
    icon,
    keywords: 'theme appearance light dark mode system colors night',
    run: () => void ctx.theme.set(pref),
  });
  const themes = (
    [
      ['system', 'shell.cmd.theme.system', 'laptop'],
      ['light', 'shell.cmd.theme.light', 'sun'],
      ['dark', 'shell.cmd.theme.dark', 'moon'],
    ] as const
  ).filter(([p]) => p !== ctx.theme.pref());
  return [
    { id: 'shell.sidebar', group: g, label: ctx.t('shell.cmd.sidebar'), icon: 'split', kbd: '[', keywords: 'sidebar collapse expand toggle focus full width', run: () => ctx.sidebar.toggle() },
    { id: 'shell.shortcuts', group: g, label: ctx.t('shell.cmd.shortcuts'), icon: 'info', kbd: '?', keywords: 'keys help hotkeys', run: () => ctx.navigate(overlay('shell.shortcuts')) },
    ...themes.map(([p, label, icon]) => theme(p, label, icon)),
    {
      id: 'shell.diagnostics',
      group: g,
      label: ctx.t('shell.cmd.diagnostics'),
      icon: 'bug',
      keywords: 'devtools debug inspector log state report',
      run: () => ctx.navigate(overlay('shell.devtools')),
    },
    {
      id: 'shell.feedback',
      group: g,
      label: ctx.t('shell.cmd.feedback'),
      icon: 'thumb-up',
      keywords: 'feedback bug report broken crash idea suggestion thumbs contact team',
      run: () => openFeedback(ctx),
    },
    { id: 'shell.home', group: g, label: ctx.t('shell.cmd.home'), icon: 'back', keywords: 'new task launcher start', run: () => ctx.navigate({ t: 'home' }) },
  ];
}

interface CrewMember {
  readonly id: string;
  readonly name: string;
  readonly role?: string;
  readonly cron?: string;
  readonly paused?: boolean;
}

/** Crew: each member (role · when they run), Org once there is a crew, Add a crew member… (v1). */
function crewItems(ctx: ShellContext): PaletteItem[] {
  const g = 'shell.palette.group.crew';
  const crew = ctx.client.entities
    .list<unknown>('crew_member')()
    .map((r) => flatRow<CrewMember>(r));
  const out: PaletteItem[] = crew.map((m) => ({
    id: `crew:${m.id}`,
    group: g,
    label: m.name,
    sub: [m.role, m.paused ? ctx.t('shell.crew.paused') : m.cron ? m.cron : ctx.t('shell.crew.whenAsked')].filter(Boolean).join(' · '),
    icon: 'crew',
    keywords: `crew agent @${m.name}`,
    run: () => ctx.navigate({ t: 'openCrew', id: m.id as never }),
  }));
  if (crew.length > 0)
    out.push({
      id: 'crew:org',
      group: g,
      label: ctx.t('shell.crew.org'),
      sub: ctx.t('shell.crew.orgSub'),
      icon: 'org',
      keywords: 'crew org chart delegates team hierarchy',
      run: () => ctx.navigate({ t: 'openView', id: 'crew.org' as never, params: null as Json }),
    });
  out.push({
    id: 'crew:add',
    group: g,
    label: ctx.t('shell.crew.addMember'),
    icon: 'plus',
    keywords: 'crew agent assistant new persistent schedule avatar',
    run: () => ctx.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'crew.editor' as never, params: {} } }),
  });
  return out;
}

/** Esc (v1 keys.js): the overlay first, then out of a text box, then Back (side, then the task). */
function escape(ctx: ShellContext, e: KeyboardEvent): boolean {
  const target = e.target as Element | null;
  if (target?.closest?.('[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"]')) return false; // it closes itself
  const nav = ctx.nav();
  if (nav.overlays.length > 0) {
    ctx.navigate({ t: 'closeOverlay' });
    return true;
  }
  if (target?.closest?.('input, textarea, select, [contenteditable="true"]')) {
    (target as HTMLElement).blur();
    return true;
  }
  if (nav.side.kind === 'none' && nav.main.kind === 'home') return false;
  ctx.navigate({ t: 'back' });
  return true;
}

/** O / Enter on the picked row: open it (or, open already, back to the list's place) */
function open(ctx: ShellContext): boolean {
  // a focused control keeps its own Enter (a button, a link, a menu item)
  const f = document.activeElement;
  if (f && f !== document.body && !f.closest('[data-part="card"]')) return false;
  const id = ctx.selection.get();
  if (!id || (ctx.nav().main.kind === 'task' && String((ctx.nav().main as { id?: unknown }).id) === id)) return false;
  // a picked row that's gone (deleted, archived away) opens nothing (audit A #11: "no task <id>")
  if (!rowIds().includes(id)) return false;
  void ctx.client.commands.run(markRead, { id, unread: false }).catch(() => undefined);
  ctx.navigate({ t: 'openTask', id: id as never });
  return true;
}

function launcher(ctx: ShellContext): void {
  if (ctx.nav().overlays.length > 0) ctx.navigate({ t: 'closeOverlay', id: ctx.nav().overlays[0]?.id as ExtensionId });
  if (!ctx.focusTargets.focus('launcher')) ctx.navigate({ t: 'home' });
}

/** The list's rows in their order on screen (the list and Home's feed draw a `card` per task with its key). */
const rowIds = (): string[] => [...document.querySelectorAll<HTMLElement>('[data-part="card"][data-id]')].map((e) => e.dataset['id'] ?? '').filter((id, i, all) => id !== '' && all.indexOf(id) === i);

/** The task the keys act on: the open one, else the row picked with J/K. */
function target(ctx: ShellContext): string | undefined {
  const main = ctx.nav().main;
  return main.kind === 'task' ? String(main.id) : ctx.selection.get();
}

/** An action of the open task's page (its Transcript, Ship it, …): handled there, as the button would. */
function taskKey(ctx: ShellContext, act: TaskKey): boolean {
  if (ctx.nav().main.kind !== 'task') return false;
  return !document.dispatchEvent(new CustomEvent(TASK_KEY, { detail: act, cancelable: true }));
}

/** J/K (v1 kbd.js): the next or previous row; with a task open, that task opens (the split follows). */
function step(ctx: ShellContext, by: 1 | -1): boolean {
  const ids = rowIds();
  if (!ids.length) return false;
  const cur = target(ctx);
  const i = cur === undefined ? -1 : ids.indexOf(cur);
  const next = ids[Math.min(ids.length - 1, Math.max(0, i < 0 ? (by > 0 ? 0 : ids.length - 1) : i + by))];
  if (!next) return false;
  // the row lights and the task's head is drawn in one pass, in the key's frame; the row is scrolled into view once that
  // frame is painted: read in the key, its layout forced the half-changed list and page (perf r2 #5, J/K 137 ms at 4×)
  batch(() => {
    ctx.selection.set(next);
    // j/k step in place (v1 replaced the address, web/js/features/feed.js syncUrl): Back leaves the task, not each step
    if (ctx.nav().main.kind === 'task') {
      markStepped();
      ctx.navigate({ t: 'openTask', id: next as never, replace: true });
    }
  });
  const show = (): void => document.querySelector<HTMLElement>(`[data-part="card"][data-id="${CSS.escape(next)}"]`)?.scrollIntoView({ block: 'nearest' });
  if (typeof requestAnimationFrame === 'function') afterNextPaint(show);
  else show();
  return true;
}

/** E, A, Delete on the open task or the picked row: as its button or menu item does. Done and Archive say so with
 *  Undo, as the row's buttons do (audit A #13); Delete asks first (audit A #4). */
function rowAct(ctx: ShellContext, act: 'done' | 'archive' | 'delete'): boolean {
  if (taskKey(ctx, act)) return true;
  const id = target(ctx);
  if (!id) return false;
  if (act === 'delete') {
    ctx.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.delete' as ExtensionId, params: { id } } });
    return true;
  }
  const run = ctx.client.commands.run;
  const undo = (message: string, back: () => void): void => void ctx.toast.success(ctx.t(message), { action: { label: ctx.t('shell.toast.undo'), onClick: back } });
  if (act === 'done') {
    const was = ctx.client.entities.peek<SessionRow>(SESSION, id as never);
    // v1 actions.js: still working, it stays put and says so (orch #18); a terminal task's Done closes its CLI
    if (doneRefused(was)) {
      ctx.toast.show(ctx.t('shell.done.working'));
      return true;
    }
    void run(setState, { id, state: 'done' }).catch(() => undefined);
    ctx.navigate({ t: 'markDone', id: id as never });
    const back = undoneState(was);
    undo('shell.toast.done', () => void run(setState, { id, state: back }).catch(() => undefined));
  } else {
    void run(archive, { id, archived: true }).catch(() => undefined);
    ctx.navigate({ t: 'archive', id: id as never });
    undo('shell.toast.archived', () => void run(archive, { id, archived: false }).catch(() => undefined));
  }
  return true;
}

/** the open task's keys that open a dialog: on the picked row (no task open) they open it for that row (audit A #12) */
const ROW_DIALOGS: Partial<Record<TaskKey, string>> = { rename: 'shell.rename', rerun: 'shell.rerun', fork: 'shell.fork' };
/** the open task's keys that need its page: on the picked row they open it, then the page does them */
const ROW_OPENS = new Set<TaskKey>(['transcript', 'openIn', 'shell', 'pr', 'changes', 'ship']);

/** A task key on the picked row while no task is open (v1 keys.js acts on the focused row): L saves it for later, M,
 *  ⇧R and ⇧F open their dialog for it, the rest open the task and do there what the key does (audit A #12). */
function rowKey(ctx: ShellContext, act: TaskKey): boolean {
  if (ctx.nav().main.kind === 'task') return false;
  const id = ctx.selection.get();
  if (!id || !rowIds().includes(id)) return false;
  const dialog = ROW_DIALOGS[act];
  if (dialog) {
    ctx.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: dialog as ExtensionId, params: { id } } });
    return true;
  }
  if (act === 'later') {
    const saved = isSaved((ctx.client.entities.peek<SessionRow>(SESSION, id as never) as { summary?: Record<string, unknown> } | undefined)?.summary?.['forLater']);
    void ctx.client.commands.run(forLater, { id, forLater: !saved }).catch(() => undefined);
    return true;
  }
  if (!ROW_OPENS.has(act)) return false;
  PENDING_KEY.id = id;
  PENDING_KEY.act = act;
  PENDING_KEY.at = Date.now();
  void ctx.client.commands.run(markRead, { id, unread: false }).catch(() => undefined);
  ctx.navigate({ t: 'openTask', id: id as never });
  return true;
}

function shortcuts(ctx: ShellContext): ShortcutDef[] {
  const k = (id: string, keys: string, label: string, act: TaskKey, group: ShortcutDef['group'] = 'task'): ShortcutDef => ({
    id: `shell.task.${id}`,
    keys,
    label,
    group,
    run: () => taskKey(ctx, act) || rowKey(ctx, act),
  });
  return [
    // the list (v1 kbd.js): J/K, O/Enter, and the row's own keys on the picked row
    { id: 'shell.next', keys: 'j', label: 'shell.key.next', group: 'list', run: () => step(ctx, 1) },
    { id: 'shell.prev', keys: 'k', label: 'shell.key.next', group: 'list', run: () => step(ctx, -1) },
    { id: 'shell.open', keys: 'o', label: 'shell.key.open', group: 'list', run: () => open(ctx) },
    { id: 'shell.open.enter', keys: 'enter', label: 'shell.key.open', group: 'list', run: () => open(ctx) },
    { id: 'shell.done', keys: 'e', label: 'shell.key.done', group: 'task', run: () => rowAct(ctx, 'done') },
    { id: 'shell.archive', keys: 'a', label: 'shell.key.archive', group: 'task', run: () => rowAct(ctx, 'archive') },
    { id: 'shell.delete', keys: 'delete', label: 'shell.key.delete', group: 'task', run: () => rowAct(ctx, 'delete') },
    { id: 'shell.delete.back', keys: 'backspace', label: 'shell.key.delete', group: 'task', run: () => rowAct(ctx, 'delete') },
    // the open task's page: what its buttons and More menu do
    k('follow', '/', 'shell.key.follow', 'reply'),
    k('later', 'l', 'shell.key.later', 'later'),
    k('rename', 'm', 'shell.key.rename', 'rename'),
    k('openIn', 't', 'shell.key.openIn', 'openIn'),
    k('shell', 'ctrl+`', 'shell.key.shell', 'shell'),
    k('pr', 'p', 'shell.key.pr', 'pr'),
    k('changes', 'd', 'shell.key.changes', 'changes'),
    k('transcript', 'v', 'shell.key.transcript', 'transcript'),
    k('rerun', 'shift+r', 'shell.key.rerun', 'rerun'),
    k('fork', 'shift+f', 'shell.key.fork', 'fork'),
    k('ship', 's', 'shell.key.ship', 'ship'),
    k('collapse', 'c', 'shell.key.collapse', 'collapse', 'general'),
    k('top', 'u', 'shell.key.top', 'top', 'general'),
    k('panel', ']', 'shell.key.panel', 'panel', 'general'),
    ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => k(`answer${n}`, String(n), 'shell.key.answer', `answer:${n}` as TaskKey)),

    { id: 'shell.palette', keys: 'mod+k', label: 'shell.key.palette', group: 'general', when: 'always', run: () => ctx.palette.open() },
    { id: 'shell.launcher', keys: 'mod+l', label: 'shell.key.launcher', group: 'general', when: 'always', run: () => launcher(ctx) },
    { id: 'shell.new', keys: 'n', label: 'shell.key.launcher', group: 'general', run: () => launcher(ctx) },
    { id: 'shell.escape', keys: 'escape', label: 'shell.key.back', group: 'general', when: 'always', run: ({ event }) => escape(ctx, event) },
    { id: 'shell.help', keys: '?', label: 'shell.key.shortcuts', group: 'general', run: () => ctx.navigate(overlay('shell.shortcuts')) },
    { id: 'shell.sidebar', keys: '[', label: 'shell.key.sidebar', group: 'general', run: () => (ctx.nav().layout === 'split' ? ctx.sidebar.toggle() : false) },
    ...projectShortcuts(ctx),
  ];
}

/** The shell's later part: its palette sources and its keys, for this shell. */
export function shellLater(ctx: ShellContext): PluginUiLater {
  return {
    palette: [
      // v1's order: tasks, crew, Actions (its head, then what plugins add at 60, then its tail), projects last
      { id: 'shell.tasks', order: 10, items: () => taskItems(ctx), deep: (q, _c, signal) => pastWork(ctx, q, signal) },
      { id: 'shell.crew', order: 20, items: () => crewItems(ctx) },
      {
        id: 'shell.new',
        order: 29,
        items: () => [
          { id: 'shell.new', group: 'shell.palette.group.actions', label: ctx.t('shell.cmd.new'), icon: 'plus', kbd: 'N', keywords: 'launch create prompt task', run: () => launcher(ctx) },
        ],
      },
      { id: PROJECT_ACTIONS, order: 30, items: () => actionHead(ctx) },
      { id: 'shell.app', order: 90, items: () => appItems(ctx) },
      { id: PROJECT_SOURCE, order: 200, items: () => projectItems(ctx) },
      // New task's place list (its selector opens the palette on it alone): nothing in ⌘K
      { id: PLACE_SOURCE, order: 201, items: (pc) => (pc.narrowed ? placeItems(ctx) : []) },
      // the switcher's last rows (Add project…, Manage projects…, Repo history): nothing in ⌘K, which has its own
      { id: PROJECT_TAIL, order: 202, items: (pc) => projectTail(ctx, pc) },
    ],
    shortcuts: shortcuts(ctx),
  };
}
