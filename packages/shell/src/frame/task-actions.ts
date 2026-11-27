// The open task's actions (TaskView.tsx): the head's bar (Changes, Ship ▾, Done, ⋯), its More menu in v1's order
// (v1 actions.js moreMenu, #786 taskMenuHtml) with what plugins attach to a task's menu, and the shell's task keys
// (v1 kbd.js), which do what those buttons and entries do.
import '../i18n/lazy.ts';
import { createMemo, onCleanup, untrack, type Accessor } from 'solid-js';
import { useSignal, type MenuEntry, type HeadAction, type TaskScroll } from '@wisp/ui';
import { sameEntries } from './same-entries.ts';
import { menuEntryOf, taskMenu, type TaskMenuEntry } from '../task-menu.ts';
import { archive, compactNow, forLater, isSaved, markRead, pin, setLean } from './commands.ts';
import { TASK_KEY, takePendingKey, type TaskKey } from './task-keys.ts';
import { displayCombo } from '../keys.ts';
import { shipsFromWorktree } from './ship-gate.ts';
import { unreadLeave } from './unread-leave.ts';
import { laterToggle } from './later-toggle.ts';
import type { EntityKey, PeerId } from '@wisp/client';
import type { useShell } from '../context.tsx';
import type { SessionRow } from '../rows.ts';
import type { compactItem } from './cold.ts';
import type { TaskSteps } from './task-step.ts';
import type { TurnBody } from './task-rows.ts';

type Shell = ReturnType<typeof useShell>;

/** What the task's actions read of the page and do through it (TaskView.tsx). */
export interface TaskActionsCtx {
  readonly shell: Shell;
  readonly t: Shell['t'];
  readonly key: EntityKey;
  /** the task's own id, the part of the key after `<peer>:` */
  readonly bare: string;
  readonly peer: PeerId;
  readonly run: Shell['client']['commands']['run'];
  readonly clients: { client(plugin: string): ReturnType<Shell['client']['plugin']> };
  readonly row: () => SessionRow | undefined;
  readonly sessions: () => readonly SessionRow[];
  readonly nav: Accessor<{ readonly layout: string; readonly side: { readonly kind: string } }>;
  readonly phoneBar: () => boolean;
  readonly busyNow: () => boolean;
  readonly lastTurn: () => (TurnBody & { id: string }) | undefined;
  readonly canRollback: () => boolean;
  readonly forLaterNow: () => boolean;
  readonly canSaveForLater: () => boolean;
  readonly compactOffer: () => ReturnType<typeof compactItem>;
  readonly hasPr: () => boolean;
  readonly toShip: () => boolean;
  readonly diff: () => { readonly files: number; readonly plus: number; readonly minus: number };
  readonly harnessName: () => string;
  readonly stepsShown: () => boolean;
  readonly steps: () => TaskSteps;
  readonly stepTo: (to: string | undefined) => void;
  readonly input: () => HTMLTextAreaElement | undefined;
  readonly scrollCtrl: () => TaskScroll | undefined;
  readonly markDone: () => void;
  readonly reopen: () => void;
  readonly next: () => void;
  readonly ship: () => void;
  readonly side: (kind: 'changes' | 'shell') => void;
  readonly sideNewest: () => void;
  readonly takeOver: () => void;
  readonly startRename: () => void;
  readonly openFind: () => void;
  readonly openFork: (turn?: string) => void;
  readonly openRerun: (agent?: string) => void;
  readonly toggleTranscript: () => void;
  readonly setTaskTranscript: (open: boolean) => void;
  readonly setOpenTurn: (id: string | undefined) => void;
  readonly setDetailsOpen: (open: boolean) => void;
  readonly setRestoreAsk: (turnId: string) => void;
}

/** The head's bar, the More menu and what its entries do; the task keys are listened for while the page is open. */
export function useTaskActions(c: TaskActionsCtx): { readonly more: Accessor<MenuEntry[]>; readonly onMore: (v: string) => void; readonly headBar: (s: SessionRow) => HeadAction[] } {
  const { shell, t, key, bare, peer, run, clients, row, sessions, nav, phoneBar, busyNow, lastTurn, canRollback, forLaterNow, canSaveForLater } = c;
  const { compactOffer, hasPr, toShip, diff, harnessName, stepsShown, steps, stepTo, input, scrollCtrl, markDone, reopen, next, ship, side, sideNewest } = c;
  const { takeOver, startRename, openFind, openFork, openRerun, toggleTranscript, setTaskTranscript, setOpenTurn, setDetailsOpen, setRestoreAsk } = c;
// v1 moreMenu: what a task's right-click menu offers is here too: the entries other plugins attach to
// `shell.row:menu` (Side discussion, Share, Ship it, Run app, Save as recipe, Add to crew, Set goal, Keep worktree);
// the shell's own (Rerun, Fork, Changes, Shell) are on this page already
const rowMenu = useSignal(shell.extensions('shell.row:menu'));
const camel = (k: string): string => k.replace(/_([a-z])/g, (_m, c: string) => c.toUpperCase());
const menuRow = (): Record<string, unknown> => {
  const r = (row() ?? {}) as unknown as Record<string, unknown>;
  const top = Object.fromEntries(
    Object.entries(r)
      .filter(([k]) => k !== 'body' && k !== 'summary')
      .map(([k, v]) => [camel(k), v]),
  );
  return { ...((r['body'] as object | undefined) ?? {}), ...top, ...((r['summary'] as object | undefined) ?? {}), id: key };
};
const extProps = (x: typeof rowMenu extends () => readonly (infer E)[] ? E : never) => ({
  client: clients.client(x.plugin),
  config: x.config,
  session: () => menuRow() as { id: string; agent: string },
});
// the shell's Rerun is More's own (above); Changes is on the head's bar (docs/ux/2026-11-18-desktop.md #4), and on a
// desktop Merge is under its Ship ▾; Shell is the row's entry here, but More's own on a phone
const canShip = (): boolean => shipsFromWorktree(row()?.summary);
const moreExts = (): ReturnType<typeof rowMenu> =>
  rowMenu().filter(
    (x) =>
      x.def.bar && !x.def.bar.rowOnly && x.id !== 'shell.row.rerun' && x.id !== 'shell.row.changes' && !(phoneBar() && x.id === 'shell.row.shell') && !(!phoneBar() && x.id === 'wisp.ship.rowMerge') && (x.def.if?.(extProps(x) as never) ?? true),
  );
// v1 #786 taskMenuHtml: the row's menu's sections and submenus (task-menu.ts), with what only More has: on a phone
// the bar's folded Transcript, Rerun and Open in first; Compact and Lean runs with the task itself; Copy resume
// command under Copy ›, Open project in editor and Restore under Files ›
const moreEntries = (): MenuEntry[] => {
  const own: TaskMenuEntry[] = [];
  // the head's bar keeps Changes, Ship, Done (docs/ux/2026-11-18-desktop.md #4): Transcript, Rerun and Open in are
  // here, with their keys, on every width
  own.push({ section: 'open', order: -30, entry: { value: 'transcript', label: t('shell.task.transcript'), icon: 'transcript', keys: 'V', checked: stepsShown() } });
  // Find in task (R4-3): a phone has no ⌘F, so it is here too
  own.push({ section: 'open', order: -25, entry: { value: 'find', label: t('shell.find.menu'), icon: 'search', keys: displayCombo('mod+f', shell.platform.apple).join('') } });
  if (!row()?.summary['sideOf'])
    own.push(
      { section: 'open', order: -20, entry: { value: 'rerun', label: t('shell.task.rerun'), icon: 'rerun', keys: '⇧R' } },
      { section: 'open', order: -10, entry: { value: 'openIn', label: t('shell.task.openIn', { harness: harnessName() }), icon: 'terminal', keys: 'T' } },
    );
  // the phone apps have no details bar: what it holds is here, after Open in (v1's order), and Details first
  if (phoneBar())
    own.push(
      { section: 'open', order: -40, entry: { value: 'details', label: t('shell.phone.details'), icon: 'info' } },
      // the side panes are views (Show › View) and the head's swipe twins go with them, so the first section stays
      // short (QA r2 #5)
      { sub: 'view', order: 10, entry: { value: 'changes', label: t('shell.task.changes'), icon: 'file', checked: nav().side.kind === 'changes' } },
      { sub: 'view', order: 20, entry: { value: 'shell', label: t('shell.task.shell'), icon: 'shell', checked: nav().side.kind === 'shell' } },
      ...(canShip() ? [{ section: 'open' as const, order: 30, entry: { value: 'ship', label: t('shell.task.ship'), icon: 'send' as const } }] : []),
      { section: 'open', order: 40, entry: { value: 'done', label: t('shell.task.done'), icon: 'done' } },
      // the head's swipe's twins: at the list's first / last task (or none) they are there, disabled
      { section: 'look', order: 200, entry: { value: 'next-task', label: t('shell.phone.nextTask'), icon: 'chevron', ...(steps().next === undefined && { disabled: true }) } },
      { section: 'look', order: 201, entry: { value: 'previous-task', label: t('shell.phone.previousTask'), icon: 'back', ...(steps().previous === undefined && { disabled: true }) } },
    );
  own.push({ section: 'task', order: -10, entry: { value: 'rename', label: t('shell.more.rename'), icon: 'pen', keys: 'M' } });
  // Pin and Mark unread, as the row's menu has them (one menu: task-menu.ts group 2)
  const pinned = row()?.summary['pinned'] === true;
  // (desktop: the phone's menu keeps the sections its QA rounds settled, task-menu.ts phoneMenu)
  if (!phoneBar()) own.push({ section: 'place', order: -40, entry: { value: 'pin', label: t(pinned ? 'shell.more.unpin' : 'shell.more.pin'), icon: 'pin' } });
  if (!phoneBar() && !busyNow() && !row()?.archived) own.push({ section: 'place', order: -35, entry: { value: 'read', label: t('shell.more.unread'), icon: 'eye' } });
  if (compactOffer()) own.push({ section: 'task', order: 45, entry: { value: 'compact', label: t('shell.cold.compact'), icon: 'minus', hint: compactOffer()?.note ?? '' } });
  // v1's task menu: Lean runs for this task, or as Settings says (Claude Code only)
  if (row()?.agent === 'claude')
    own.push({
      section: 'task',
      order: 46,
      entry: {
        kind: 'group',
        id: 'lean',
        label: t('shell.task.lean'),
        items: (['follow', 'on', 'off'] as const).map((v) => ({
          value: `lean:${v}`,
          label: t(`shell.task.lean.${v}`),
          checked: ((row()?.body as { lean?: string } | undefined)?.lean ?? 'follow') === v,
        })),
      },
    });
  own.push({ sub: 'copy', order: -20, entry: { value: 'copy-id', label: t('shell.more.copyId'), icon: 'copy' } });
  // Copy resume command and Open project in editor are the shell's row entries (row-menu.ts), here as on the row
  if (canRollback()) own.push({ sub: 'files', order: 130, entry: { value: 'rollback', label: t('shell.more.rollback'), icon: 'replay' } });
  if (forLaterNow()) own.push({ section: 'place', order: -20, entry: { value: 'later', label: t('shell.more.unlater'), icon: 'bookmark', keys: 'L' } });
  else if (canSaveForLater()) own.push({ section: 'place', order: -20, entry: { value: 'later', label: t('shell.more.later'), icon: 'bookmark', keys: 'L' } });
  if (row()?.archived) own.push({ section: 'place', order: -10, entry: { value: 'unarchive', label: t('shell.more.unarchive'), icon: 'archive', keys: 'A' } });
  else if (!busyNow()) own.push({ section: 'place', order: -10, entry: { value: 'archive', label: t('shell.task.archive'), icon: 'archive', keys: 'A' } });
  return taskMenu({
    entries: [...own, ...moreExts().map((x) => menuEntryOf(x, extProps(x) as never, t))],
    danger: [{ value: 'delete', label: t('shell.more.delete'), icon: 'trash', danger: true, keys: '⌫' }],
    t,
    // the phone apps' ⋯: short, its longer sections behind one row each (QA r2 #5)
    ...(phoneBar() && { compact: true }),
  });
};
// the same array while the menu says the same: every patch to the open task (a streamed token) built new entries, and
// the kit, keying rows by object, rebuilt the closed menu each time (perf r1 #12, #15)
const more = createMemo(moreEntries, [], { equals: sameEntries });
const onMore = (v: string): void => {
  if (v === 'next-task') stepTo(steps().next);
  else if (v === 'previous-task') stepTo(steps().previous);
  else if (v === 'transcript') toggleTranscript();
  else if (v === 'find') openFind();
  else if (v === 'rerun') openRerun();
  else if (v === 'openIn') takeOver();
  else if (v === 'shell') side('shell');
  else if (v === 'details') setDetailsOpen(true);
  else if (v === 'changes') side('changes');
  else if (v === 'ship') ship();
  else if (v === 'done') markDone();
  else if (v === 'fork') openFork();
  else if (v === 'compact') void run(compactNow, { id: key }).catch(() => undefined);
  else if (v.startsWith('lean:')) {
    const lean = v.slice(5);
    void run(setLean, { id: key, ...(lean !== 'follow' && { lean: lean as 'on' | 'off' }) }).catch(() => undefined);
  } else if (v === 'copy') void shell.platform.copy(location.href).then((ok) => shell.toast.show(t(ok ? 'shell.task.copied' : 'shell.task.copyFailed')));
  else if (v === 'archive') {
    void run(archive, { id: key, archived: true }).catch(() => undefined);
    shell.navigate({ t: 'archive', id: key as never });
    next();
    // with Undo, as the row's Archive (audit A #13)
    shell.toast.success(t('shell.toast.archived'), { action: { label: t('shell.toast.undo'), onClick: () => void run(archive, { id: key, archived: false }).catch(() => undefined) } });
  } else if (v === 'delete')
    // asked first, as the row's Delete (audit A #4): the shell's one Delete task dialog
    shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.delete' as never, params: { id: key } } }); else if (v === 'rename') startRename();
  else if (v === 'copy-id') void shell.platform.copy(bare).then((ok) => ok && shell.toast.show(t('shell.more.copiedId')));
  else if (v === 'later') {
    // with Undo, as the row menu and the hover button say it (audit B #7)
    const l = laterToggle(row()?.summary['forLater']);
    void run(forLater, { id: key, ...l.next }).catch(() => undefined);
    shell.toast.show(t(l.toast), { action: { label: t('shell.toast.undo'), onClick: () => void run(forLater, { id: key, ...l.undo }).catch(() => undefined) } });
  }
  else if (v === 'pin') void run(pin, { id: key, pinned: row()?.summary['pinned'] !== true }).catch(() => undefined);
  else if (v === 'read') {
    // Mark unread: it stays bold in the list, so the page leaves it (staying would read it again): the next task that
    // needs you, else home, saying so with Undo (audit B #5: a silent jump read as the task changing under you)
    void run(markRead, { id: key, unread: true }).catch(() => undefined);
    const out = unreadLeave(untrack(sessions), key, row()?.title ?? '', peer);
    shell.navigate(out.to.t === 'openTask' ? { t: 'openTask', id: out.to.id as never } : { t: 'home' });
    shell.toast.success(t(out.toast.key, out.toast.vars), {
      action: {
        label: t('shell.toast.undo'),
        onClick: () => {
          void run(markRead, { id: key, unread: false }).catch(() => undefined);
          shell.navigate({ t: 'openTask', id: key as never });
        },
      },
    });
  }
  else if (v === 'unarchive') void run(archive, { id: key, archived: false }).catch(() => undefined);
  else if (v === 'rollback') {
    const l = lastTurn();
    if (l) setRestoreAsk(l.id);
  } else if (v.startsWith('x:')) {
    const x = rowMenu().find((e) => `x:${e.id}` === v);
    if (x) x.def.bar?.select?.(extProps(x) as never);
  }
};
// Ship ▾: Ship it…, the PR's pane, Merge when ready (wisp.ship's row entry, when it offers it); Ship it… alone is a button
const shipMenu = (): MenuEntry[] => {
  const merge = rowMenu().find((x) => x.id === 'wisp.ship.rowMerge' && (x.def.if?.(extProps(x) as never) ?? true));
  const mergeLabel = merge?.def.bar?.text?.(extProps(merge) as never) ?? (merge?.def.bar?.label ? t(merge.def.bar.label) : undefined);
  return [
    { value: 'ship', label: t('shell.task.ship'), icon: 'send', keys: 'S' },
    ...(hasPr() ? [{ value: 'pr', label: t('shell.task.openPr'), icon: 'branch' as const, keys: 'P' }] : []),
    ...(mergeLabel ? [{ value: 'x:wisp.ship.rowMerge', label: mergeLabel, icon: 'merge' as const }] : []),
  ];
};
const onShip = (v: string): void => {
  if (v === 'pr') shell.navigate({ t: 'toggleSide', side: { kind: 'pr', id: key as never } });
  else onMore(v);
};
// the head's one bar (docs/ux/2026-11-18-desktop.md #4), the same with and without the panel: Changes +n −n, Ship ▾,
// Done (Reopen when done), ⋯; Transcript, Rerun, Open in and Shell are in ⋯ with their keys
const headBar = (s: SessionRow): HeadAction[] => {
  const d = diff();
  const ship = shipMenu();
  const done = s.state === 'done';
  return [
    // a diff the turns made (with its figures), or work the worktree holds that they don't show (dirty, ahead)
    ...(d.files > 0 || toShip()
      ? [{ id: 'changes', key: 'D', icon: 'file' as const, label: t('shell.task.changes'), title: t('shell.task.changesTitle'), part: 'split.changes', ...(d.files > 0 && { figures: { plus: `+${d.plus}`, minus: `−${d.minus}` } }), act: { onClick: () => side('changes') } }]
      : []),
    // only a worktree task ships (v1 shippable; audit B #2: one in its project's folder failed "no such session")
    ...(canShip()
      ? [
          {
            id: 'ship',
            key: 'S',
            icon: 'send' as const,
            label: t('shell.task.shipShort'),
            title: t('shell.task.shipTitle'),
            part: 'split.ship',
            act: ship.length > 1 ? { menu: { items: ship, onSelect: onShip } } : { onClick: () => onMore('ship') },
          },
        ]
      : []),
    done
      ? { id: 'done', key: 'E', icon: 'replay', label: t('shell.task.reopen'), title: t('shell.task.reopenTitle'), part: 'split.done', act: { onClick: reopen } }
      : { id: 'done', key: 'E', icon: 'done', label: t('shell.task.doneShort'), title: t('shell.card.done'), tone: 'good', part: 'split.done', act: { onClick: markDone } },
    { id: 'more', icon: 'dots', label: t('shell.task.more'), title: t('shell.task.more'), part: 'split.more', bare: true, act: { menu: { items: more(), onSelect: onMore } } },
  ];
};
// the shell's task keys (v1 kbd.js, builtins.tsx): what this page's buttons and More menu do
const onKey = (e: Event): void => {
  const act = (e as CustomEvent<TaskKey>).detail;
  const did = ((): boolean => {
    if (act === 'transcript') toggleTranscript();
    else if (act === 'find') openFind();
    else if (act === 'rerun' || act === 'fork' || act === 'archive' || act === 'delete') onMore(act);
    else if (act === 'openIn') takeOver();
    else if (act === 'shell') side('shell');
    else if (act === 'changes' || act === 'panel') {
      if (act === 'panel' && nav().side.kind !== 'none') shell.navigate({ t: 'closeSide' });
      else if (act === 'panel') sideNewest();
      else side('changes');
    } else if (act === 'pr') shell.navigate({ t: 'toggleSide', side: { kind: 'pr', id: key as never } });
    else if (act === 'ship') {
      if (!canShip()) return false;
      ship();
    }
    else if (act === 'done') {
      if (row()?.state === 'done') reopen();
      else markDone();
    } else if (act === 'later') void run(forLater, { id: key, forLater: !isSaved(row()?.summary['forLater']) }).catch(() => undefined);
    else if (act === 'reply') input()?.focus();
    else if (act === 'rename') startRename();
    else if (act === 'collapse') {
      setTaskTranscript(false);
      setOpenTurn(undefined);
    } else if (act === 'top') {
      const sc = scrollCtrl();
      if (sc) sc.place(0, false);
      else document.querySelector('[data-kit="turns"]')?.firstElementChild?.scrollIntoView({ block: 'start' });
    }
    else if (act.startsWith('answer:')) {
      // the question waiting over the reply box, else the latest answer's question block: its Nth choice
      const n = Number(act.slice(7)) - 1;
      const opts = document.querySelectorAll<HTMLButtonElement>('[data-part="task.ask"] button, [data-part="block.question.option"]:not(:disabled)');
      const b = opts[n];
      if (!b) return false;
      b.click();
    } else return false;
    return true;
  })();
  if (did) e.preventDefault();
};
document.addEventListener(TASK_KEY, onKey);
onCleanup(() => document.removeEventListener(TASK_KEY, onKey));
// a key pressed on this task's row in the list (V, T, P, D, S, ⌃`): the list opened it, the page does it (audit A #12)
const pendingAct = takePendingKey(key, Date.now());
if (pendingAct) queueMicrotask(() => onKey(new CustomEvent(TASK_KEY, { detail: pendingAct, cancelable: true })));
  return { more, onMore, headBar };
}
