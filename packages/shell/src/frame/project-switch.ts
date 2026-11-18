// Switching project (owner, 2026-11-12): one selector in the top bar instead of a row of chips, and a switcher you reach
// from anywhere, as fast tools do it (Linear's workspace switcher, Slack's ⌘K, VS Code's Open Recent, JetBrains'
// project switcher):
//   - ⌘P (Ctrl+P) opens the palette on projects only: type to filter, ↑↓ and Enter, recent first;
//   - ⌘1…⌘9 go straight to your first nine projects in your own order (v1 #772's order), ⌘0 to All projects;
//   - ⌘K lists them too, under Projects, so one palette finds everything.
// Each row says how many of its tasks need you, so the switcher doubles as "where am I needed".

import type { PaletteItem, ShellContext, ShortcutDef } from '../contract.ts';
import { SESSION, type SessionRow } from '../rows.ts';
import { displayCombo } from '../keys.ts';
import { PROJECT_ORDER, needsYou, projectList, projectOfRow, type ProjectView } from './rows-view.ts';
import { parsePick, pickOnly, pickToggle, pickValue } from './project-pick.ts';
import { projectLabels } from './project-labels.ts';

/** The palette source's id: the switcher opens the palette on it alone. */
export const PROJECT_SOURCE = 'shell.projects';
/** ⌘K's Actions head (New task, Add project…, Manage projects): the switcher lists it under its projects. */
export const PROJECT_ACTIONS = 'shell.actions';
/** The switcher's last rows: Add project…, Manage projects… and the repo's history (docs/ux/2026-11-18-desktop.md #11:
 *  the bare + beside "All ▾" went, its menu is here). Only in the switcher (a narrowed palette); ⌘K has its own. */
export const PROJECT_TAIL = 'shell.projects.tail';

export function projectTail(ctx: ShellContext, pc: { readonly narrowed?: boolean | undefined }): PaletteItem[] {
  if (!pc.narrowed) return [];
  const g = ctx.t('shell.palette.group.projects');
  const projects = (): void => ctx.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.projects' as never, params: null } });
  return [
    { id: 'project:add', group: g, label: ctx.t('shell.cmd.addProject'), icon: 'folder', keywords: 'new project folder repo directory', run: projects },
    { id: 'project:manage', group: g, label: ctx.t('shell.side.manageMore'), icon: 'settings', keywords: 'order rename colour color remove', run: projects },
    // a project's committed agent history (wisp.history's Repo view), when that plugin is on
    ...(ctx.view('history.repo') ? [{ id: 'project:repo', group: g, label: ctx.t('shell.side.repo'), icon: 'branch', keywords: 'history repo commits agents', run: () => ctx.navigate({ t: 'openView', id: 'history.repo' as never, params: {} }) }] : []),
  ];
}

/** The projects in your order, from the store as it is now. */
export function projectsNow(ctx: ShellContext): readonly ProjectView[] {
  const rows = ctx.client.entities.list<unknown>('project')();
  const order = ctx.client.entities
    .list<{ key: string; value: unknown }>('setting')()
    .find((s) => s.key === PROJECT_ORDER)?.value;
  return projectList(rows, order);
}

/** How many tasks in each project need you, and in all of them. */
export function needCounts(ctx: ShellContext, projects: readonly ProjectView[]): { all: number; by: ReadonlyMap<string, number> } {
  const by = new Map<string, number>();
  let all = 0;
  for (const s of ctx.client.entities.list<SessionRow>(SESSION)()) {
    // the list's Needs you, and a worktree run in the project it was made from (QA round 1 #9)
    if (!needsYou(s)) continue;
    all++;
    const p = projectOfRow(s, projects);
    if (p) by.set(p.id, (by.get(p.id) ?? 0) + 1);
  }
  return { all, by };
}

/** What the list shows now: 'all', one project's id, or several joined by commas (project-pick.ts). */
export function currentProject(ctx: ShellContext): string {
  return ctx.projectFilter?.value() ?? 'all';
}

/** The projects picked now, in the order added ([] for All projects). */
export const currentPick = (ctx: ShellContext): string[] => parsePick(currentProject(ctx));

/** Show only `id` ('all' for every project) in the list: a row's name, Enter, ⌘1–⌘9. */
export function switchTo(ctx: ShellContext, id: string): void {
  ctx.projectFilter?.set(pickValue(pickOnly(id)));
}

/** Add `id` to the pick, or take it out (a row's + / −, ⌘-Enter); the menu stays open. */
export function togglePick(ctx: ShellContext, id: string): void {
  ctx.projectFilter?.set(pickValue(pickToggle(currentPick(ctx), id)));
}

/** Set the whole pick (a strip chip's ×). */
export function setPick(ctx: ShellContext, ids: readonly string[]): void {
  ctx.projectFilter?.set(pickValue(ids));
}

/** The switcher: the palette on projects alone. */
export function openSwitcher(ctx: ShellContext): void {
  ctx.navigate({
    t: 'openOverlay',
    overlay: {
      kind: 'dialog',
      id: 'shell.palette' as never,
      params: { only: [PROJECT_SOURCE, PROJECT_TAIL], placeholder: ctx.t('shell.projects.switchPlaceholder'), title: ctx.t('shell.projects.switch') },
    },
  });
}

/** The palette's Projects group: All projects, then yours in your order, then add and manage. */
export function projectItems(ctx: ShellContext): PaletteItem[] {
  const projects = projectsNow(ctx);
  const { all, by } = needCounts(ctx, projects);
  const pick = currentPick(ctx);
  const several = pick.length > 1;
  const g = ctx.t('shell.palette.group.projects');
  const apple = ctx.platform.apple;
  const need = (n: number): string | undefined => (n > 0 ? ctx.t(n === 1 ? 'shell.side.need.one' : 'shell.side.need.other', { count: n }) : undefined);
  const subOf = (n: number, current: boolean, path?: string): string | undefined => {
    const parts = [current ? ctx.t('shell.projects.current') : undefined, need(n), path].filter((x): x is string => !!x);
    return parts.length > 0 ? parts.join(' · ') : undefined;
  };
  const allSub = subOf(all, pick.length === 0);
  const out: PaletteItem[] = [
    {
      id: 'project:all',
      group: g,
      label: ctx.t('shell.side.all'),
      icon: 'feed',
      ...(allSub !== undefined && { sub: allSub }),
      kbd: displayCombo('mod+0', apple).join(' '),
      keywords: 'all every projects',
      run: () => switchTo(ctx, 'all'),
    },
  ];
  out.push(
    ...projectRows(ctx, projects, by, pick, {
      kbd: true,
      sub: (inPick) => (inPick && !several ? ctx.t('shell.projects.current') : undefined),
      only: (id) => switchTo(ctx, id),
      toggle: (id) => togglePick(ctx, id),
    }),
  );
  return out; // adding and managing them are Actions (v1), which the switcher lists too
}

/** One row per project, as the switcher and New task's place list both show them: the colour's dot, the name (two of
 *  the same name say where each is), what it says (current, need-you count, folder), ⌘1–⌘9 (the switcher only); its
 *  name = only this one, its + / − = add it or take it out (the palette stays open). */
function projectRows(
  ctx: ShellContext,
  projects: readonly ProjectView[],
  by: ReadonlyMap<string, number>,
  pick: readonly string[],
  o: { readonly kbd: boolean; readonly sub: (inPick: boolean, at: number) => string | undefined; readonly only: (id: string) => void; readonly toggle: (id: string) => void },
): PaletteItem[] {
  const g = ctx.t('shell.palette.group.projects');
  const apple = ctx.platform.apple;
  const names = projectLabels(projects, (peer) => ctx.client.peers.name(peer as never));
  return projects.map((p, i) => {
    const inPick = pick.includes(p.id);
    const need = by.get(p.id) ?? 0;
    const parts = [o.sub(inPick, pick.indexOf(p.id)), need > 0 ? ctx.t(need === 1 ? 'shell.side.need.one' : 'shell.side.need.other', { count: need }) : undefined, p.path].filter((x): x is string => !!x);
    return {
      id: `project:${p.id}`,
      group: g,
      label: names[i] ?? p.name,
      color: p.color || 'slate',
      ...(parts.length > 0 && { sub: parts.join(' · ') }),
      ...(o.kbd && i < 9 && { kbd: displayCombo(`mod+${i + 1}`, apple).join(' ') }),
      keywords: `project ${p.path}`,
      run: () => o.only(p.id),
      // the + at the row's end adds it to the pick without closing; once in, a check and a −
      toggle: { on: inPick, label: ctx.t(inPick ? 'shell.pick.remove' : 'shell.pick.add', { name: p.name, key: displayCombo('mod+enter', apple).join('') }), run: () => o.toggle(p.id) },
    };
  });
}

/** New task's place (owner, 2026-11-15): what its selector picks, held by the prompt box that opened the list. */
export interface PlaceTarget {
  /** the projects picked, the one it starts in first; [] for No folder */
  pick(): readonly string[];
  set(ids: readonly string[]): void;
  /** Other folder…: a folder that isn't a project yet */
  other(): void;
}

/** New task's place list: a palette source that answers only while a prompt box has it open (never in ⌘K). */
export const PLACE_SOURCE = 'shell.places';
let place: PlaceTarget | undefined;

/** New task's selector: the same list as ⌘P (search, dots, need-you counts, + adds) on this box's place, with No folder
 *  and Other folder… after the projects. */
export function openPlaces(ctx: ShellContext, target: PlaceTarget): void {
  place = target;
  ctx.navigate({
    t: 'openOverlay',
    overlay: { kind: 'dialog', id: 'shell.palette' as never, params: { only: [PLACE_SOURCE], placeholder: ctx.t('shell.home.placeSearch'), title: ctx.t('shell.home.placeTitle') } },
  });
}

/** The prompt box is gone: its list no longer answers for it. */
export function releasePlaces(target: PlaceTarget): void {
  if (place === target) place = undefined;
}

/** The place list: the projects (name = only it, + adds it; the first picked is where it starts), No folder, Other
 *  folder…. */
export function placeItems(ctx: ShellContext): PaletteItem[] {
  const target = place;
  // opened again with no prompt box behind it (a reload, Back to its URL): nothing to pick for, so it closes
  if (!target) {
    queueMicrotask(() => ctx.navigate({ t: 'closeOverlay', id: 'shell.palette' as never }));
    return [];
  }
  const projects = projectsNow(ctx);
  const { by } = needCounts(ctx, projects);
  const pick = target.pick();
  const g = ctx.t('shell.palette.group.projects');
  return [
    ...projectRows(ctx, projects, by, pick, {
      kbd: false,
      sub: (inPick, at) => (!inPick ? undefined : at === 0 && pick.length > 1 ? ctx.t('shell.home.startsHere') : ctx.t('shell.home.picked')),
      only: (id) => target.set(pickOnly(id)),
      toggle: (id) => target.set(pickToggle(target.pick(), id)),
    }),
    { id: 'place:none', group: g, label: ctx.t('shell.home.noFolder'), icon: 'text', ...(pick.length === 0 && { sub: ctx.t('shell.home.picked') }), keywords: 'none scratch general', run: () => target.set([]) },
    { id: 'place:other', group: g, label: ctx.t('shell.home.otherFolder'), icon: 'folder', keywords: 'other folder add browse', run: () => target.other() },
  ];
}

/** ⌘P opens the switcher; ⌘1…⌘9 go to your first nine projects, ⌘0 to All projects. */
export function projectShortcuts(ctx: ShellContext): ShortcutDef[] {
  const go = (n: number): boolean => {
    if (n === 0) {
      switchTo(ctx, 'all');
      return true;
    }
    const p = projectsNow(ctx)[n - 1];
    if (!p) return false;
    switchTo(ctx, p.id);
    return true;
  };
  return [
    { id: 'shell.projects.switch', keys: 'mod+p', label: 'shell.key.projects', group: 'general', when: 'always', run: () => openSwitcher(ctx) },
    ...[0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((n): ShortcutDef => ({
      id: `shell.projects.go${n}`,
      keys: `mod+${n}`,
      label: n === 0 ? 'shell.key.projectAll' : 'shell.key.projectN',
      group: 'general',
      when: 'always',
      run: () => go(n),
    })),
  ];
}
