// The task's ⋮ / ⋯ on a phone (docs/ux/2026-11-18-phone.md #6): the task menu the browser builds (TaskView's more(),
// task-menu.ts), regrouped into three groups (HIG: a menu of about three groups; M3: short menus): what you do with it
// now (Ship it…, Mark done), working on it (Rerun…, Open in, then one *This task* submenu with everything else, each
// group headed as it was), and putting it away (Save for later, Archive, Delete). Every entry of the browser's menu is
// still there, once; the views (Changes, Shell, the PR) are also the page's status pills and the side sheet's tabs.
// Pure, over whatever more() holds, so a new plugin entry lands in *This task* without a change here.
import '../i18n/lazy.ts';
import { menuSections, type MenuEntry, type MenuGroup, type MenuItem, type MenuSubmenu } from '@wisp/ui/pure';

const ACT = ['ship', 'done', 'reopen'];
const WORK = ['rerun', 'openIn'];
const AWAY = ['later', 'archive', 'unarchive', 'delete'];

type Leaf = MenuItem | MenuGroup;

/** every entry as it would be picked: submenus opened into headed groups, separators gone */
function leaves(es: readonly MenuEntry[]): Leaf[] {
  return es.flatMap((e): Leaf[] => {
    if (e.kind === 'separator') return [];
    if (e.kind === 'submenu') {
      const inner = leaves(e.items);
      // a submenu that was one choice of several (Lean runs, Copy) stays a headed group; nested ones flatten into it
      const items = inner.flatMap((x): MenuItem[] => (x.kind === 'group' ? [...x.items] : [x]));
      return items.length ? [{ kind: 'group', id: e.value, label: e.label, items }] : [];
    }
    return [e];
  });
}

const valueOf = (x: Leaf): string => (x.kind === 'group' ? `group:${x.id}` : x.value);

export function phoneTaskMenu(full: readonly MenuEntry[], t: (key: string) => string): MenuEntry[] {
  const all = leaves(full);
  const pick = (values: readonly string[]): Leaf[] => values.flatMap((v) => all.filter((x) => valueOf(x) === v));
  const act = pick(ACT);
  const work = pick(WORK);
  const away = pick(AWAY);
  const taken = new Set([...act, ...work, ...away]);
  const rest = all.filter((x) => !taken.has(x));
  const task: MenuSubmenu | undefined = rest.length
    ? {
        kind: 'submenu',
        value: 'section:task',
        label: t('shell.phone.thisTask'),
        icon: 'pen',
        items: rest,
      }
    : undefined;
  return menuSections([act, [...work, ...(task ? [task] : [])], away]);
}
