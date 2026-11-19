// Audit F setup F3: with four projects the switcher (⌘P) listed All projects, the projects and Add project…, and hid
// Manage projects… and Repo history until you typed; from six, Add project… went too, and from the seventh, projects
// ⌘6–⌘9 reach. An empty query showed each group's first 6, and the projects and their tail share one group.
import { expect, test } from 'vitest';
import type { PaletteItem } from '../src/contract.ts';
import { createRanker } from '../src/palette/match.ts';

const g = 'Projects';
const items: PaletteItem[] = [
  { id: 'project:all', group: g, label: 'All projects' },
  ...Array.from({ length: 9 }, (_, i): PaletteItem => ({ id: `project:p${i + 1}`, group: g, label: `repo${i + 1}` })),
  { id: 'project:add', group: g, label: 'Add project…' },
  { id: 'project:manage', group: g, label: 'Manage projects…' },
  { id: 'project:repo', group: g, label: 'Repo history' },
];

test('the_switcher_lists_every_project_and_its_tail_with_an_empty_query_audit_f_setup_f3', () => {
  const ids = createRanker({ whole: true })(items, '', 0).map((r) => r.item.id);
  expect(ids).toEqual(items.map((i) => i.id));
  // ⌘K (not narrowed) keeps its 6 per group
  expect(createRanker()(items, '', 0)).toHaveLength(6);
  // typing ranks as ever
  expect(createRanker({ whole: true })(items, 'manage', 0)[0]?.item.id).toBe('project:manage');
});
