// The task's ⋮ / ⋯ on a phone (docs/ux/2026-11-18-phone.md #6, QA r6 #6): the task menu the browser builds (TaskView's
// more(), task-menu.ts) in three groups of nine rows (HIG: about three groups; M3: short menus): what you do with it now
// (Ship it…, Mark done), working on it (Rerun…, then This task ›, Show ›, Keep and copy ›), and putting it away (Save
// for later, Archive, Delete). Every entry of the browser's menu is still there, once, in the submenu its section says:
// This task holds the task itself (Rename, Details, Open in, the goal, the crew, Pin…), Show what you look at (the
// transcript, the side panes, the next task), Keep and copy what you take away (Fork, copies, files, Share). A choice
// of several (Lean runs) keeps its name on each row, as a phone's drill-in shows no group heading; a plain group (View)
// is just its rows. Pure.
import '../i18n/lazy.ts';
import { menuSections, type MenuEntry, type MenuItem, type MenuSubmenu } from '@wisp/ui/pure';

const ACT = ['ship', 'done', 'reopen'];
const WORK = ['rerun'];
const AWAY = ['later', 'archive', 'unarchive', 'delete'];
const SHOW = new Set(['transcript', 'changes', 'shell', 'pr', 'next-task', 'previous-task']);
const KEEP = new Set(['fork', 'x:shell.row.fork', 'copy-id', 'x:shell.row.copyResume', 'rollback']);
/** a plugin's entry that belongs with the task even when its section is Show (Set goal, Add to crew) */
const OWN = /goal|crew/i;

interface Leaf {
  readonly item: MenuItem;
  /** the browser menu's section it came from ('section:look', 'sub:copy'), or '' at the top */
  readonly from: string;
}

/** every row as it would be picked, with where it came from; a group's name goes on its rows */
function leaves(es: readonly MenuEntry[], from = ''): Leaf[] {
  return es.flatMap((e): Leaf[] => {
    if (e.kind === 'separator') return [];
    if (e.kind === 'submenu') return leaves(e.items, from || e.value);
    if (e.kind === 'group') {
      // a choice among the group's own values (Lean runs: lean:follow, lean:on…) keeps its name on each row, its rows
      // saying nothing alone ("On for this task"); a group of rows that name themselves (View: Changes ✓, Shell) is them
      const choice = e.items.every((item) => item.value.startsWith(`${e.id}:`));
      return e.items.map((item) => ({ item: choice ? { ...item, label: `${e.label} · ${item.label}` } : item, from: from || e.id }));
    }
    return [{ item: e, from }];
  });
}

type Bucket = 'act' | 'work' | 'away' | 'task' | 'show' | 'keep';

function bucketOf(l: Leaf): Bucket {
  const v = l.item.value;
  if (ACT.includes(v)) return 'act';
  if (WORK.includes(v)) return 'work';
  if (AWAY.includes(v)) return 'away';
  if (OWN.test(v)) return 'task';
  if (SHOW.has(v) || l.from === 'section:look' || l.from === 'sub:view') return 'show';
  if (KEEP.has(v) || /share/i.test(v) || l.from === 'section:keep' || l.from === 'sub:copy' || l.from === 'sub:files') return 'keep';
  return 'task';
}

const ORDER: Readonly<Record<string, readonly string[]>> = { act: ACT, work: WORK, away: AWAY };
const byOrder = (xs: readonly MenuItem[], order: readonly string[]): MenuItem[] => [...xs].sort((a, b) => order.indexOf(a.value) - order.indexOf(b.value));

export function phoneTaskMenu(full: readonly MenuEntry[], t: (key: string) => string): MenuEntry[] {
  const all = leaves(full);
  const of = (b: Bucket): MenuItem[] => all.filter((l) => bucketOf(l) === b).map((l) => l.item);
  const sub = (b: 'task' | 'show' | 'keep', key: string, icon: MenuSubmenu['icon']): MenuSubmenu[] => {
    const items = of(b);
    return items.length ? [{ kind: 'submenu', value: `section:${b}`, label: t(key), ...(icon && { icon }), items }] : [];
  };
  return menuSections([
    byOrder(of('act'), ORDER['act'] ?? []),
    [...of('work'), ...sub('task', 'shell.phone.thisTask', 'pen'), ...sub('show', 'shell.phone.show', 'panel'), ...sub('keep', 'shell.phone.keep', 'bookmark')],
    byOrder(of('away'), ORDER['away'] ?? []),
  ]);
}
