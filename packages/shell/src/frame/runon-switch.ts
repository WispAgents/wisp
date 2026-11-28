// Where a task runs, picked as its project is (owner, 2026-11-27): New task's "This computer · Worktree" moved from
// the footer's menu to a selector beside the project's, and it opens the same palette dialog as the project's (search,
// ↑↓, Enter): the computers (this one, the ones you added, the places plugins add), Add a computer…, then the folder's
// Worktree / In place. The list's head has the same kind of selector to show one computer's tasks (`computerFilter`).

import type { PaletteItem, ShellContext } from '../contract.ts';
import { ALL_COMPUTERS, knownComputer } from './computer-filter.ts';
import { sandboxPeers } from './where.ts';
import { hubName } from './here.ts';

/** New task's Run on list: a palette source that answers only while a prompt box has it open (never in ⌘K). */
export const RUNON_SOURCE = 'shell.runon';
/** The list's computer filter: the palette on the computers alone (never in ⌘K). */
export const COMPUTER_SOURCE = 'shell.computers';

/** What the prompt box that opened the Run on list offers; it builds the rows, as it knows its computers and pick. */
export interface RunOnTarget {
  items(): PaletteItem[];
}

let runOn: RunOnTarget | undefined;

/** New task's Run on selector: the palette on its list. `title` comes from Home (its string is Home's, en.home.json:
 * this module is also the Sidebar's, so it never loads them). */
export function openRunOn(ctx: ShellContext, target: RunOnTarget, title: string): void {
  runOn = target;
  ctx.navigate({
    t: 'openOverlay',
    overlay: { kind: 'dialog', id: 'shell.palette' as never, params: { only: [RUNON_SOURCE], placeholder: ctx.t('shell.runon.search'), title } },
  });
}

/** The prompt box is gone: its list no longer answers for it. */
export function releaseRunOn(target: RunOnTarget): void {
  if (runOn === target) runOn = undefined;
}

export function runOnItems(ctx: ShellContext): PaletteItem[] {
  const target = runOn;
  // opened again with no prompt box behind it (a reload, Back to its URL): nothing to pick for, so it closes
  if (!target) {
    queueMicrotask(() => ctx.navigate({ t: 'closeOverlay', id: 'shell.palette' as never }));
    return [];
  }
  return target.items();
}

/** The computers a task can be on: this one first, then every other the client has, never a task's sandbox. */
export function computersNow(ctx: ShellContext): string[] {
  const home = ctx.client.home();
  const boxes = sandboxPeers(ctx.client.entities.list<{ id: unknown; body?: { data?: unknown } }>('machines_peer')());
  const peers = ctx.client.peers.list()().filter((p) => !boxes.has(p));
  return [...(peers.includes(home) ? [home] : []), ...peers.filter((p) => p !== home)];
}

/** A computer's name: This computer for the one this app runs on. */
export function computerName(ctx: ShellContext, peer: string): string {
  return peer === ctx.client.home() ? (hubName(ctx.client) ?? ctx.t('shell.card.thisComputer')) : ctx.client.peers.name(peer as never) || peer;
}

/** The list's computer filter: the palette on the computers alone. */
export function openComputers(ctx: ShellContext): void {
  ctx.navigate({
    t: 'openOverlay',
    overlay: { kind: 'dialog', id: 'shell.palette' as never, params: { only: [COMPUTER_SOURCE], placeholder: ctx.t('shell.computers.search'), title: ctx.t('shell.computers.title') } },
  });
}

/** All computers, then each computer; the one shown says Current. */
export function computerItems(ctx: ShellContext): PaletteItem[] {
  const peers = computersNow(ctx);
  const on = knownComputer(ctx.computerFilter?.value(), peers);
  const g = ctx.t('shell.computers.group');
  const current = ctx.t('shell.projects.current');
  const set = (id: string): void => ctx.computerFilter?.set(id);
  return [
    { id: 'computer:all', group: g, label: ctx.t('shell.phone.allComputers'), icon: 'feed', rank: 0, ...(on === ALL_COMPUTERS && { sub: current }), keywords: 'all every computers machines', run: () => set(ALL_COMPUTERS) },
    ...peers.map(
      (p, i): PaletteItem => ({
        id: `computer:${p}`,
        group: g,
        label: computerName(ctx, p),
        icon: p === ctx.client.home() && !hubName(ctx.client) ? 'laptop' : 'server',
        rank: i + 1,
        ...(on === p && { sub: current }),
        keywords: `computer machine ${ctx.client.peers.name(p as never)}`,
        run: () => set(p),
      }),
    ),
  ];
}
