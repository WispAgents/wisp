// @vitest-environment happy-dom
// QA round 1 #7: after Save for later (or Archive) the row's menu still offered "Save for later" ("Mark done / Unsave /
// Archive" on an archived row) until a reload. The menu read its row with an untracked peek inside a Solid memo, so it
// was built once per opened row and never again when the row changed.
import { expect, test } from 'vitest';
import { createRoot } from 'solid-js';
import { signal, untracked, type EntityKey } from '@wisp/client';
import { useDerived, type MenuEntry } from '@wisp/ui';
import { menuFor } from '../src/ui/TaskList.tsx';

const key = 'local:s1' as EntityKey;
const raw = (summary: Record<string, unknown>, more: Record<string, unknown> = {}) => ({ id: 's1', title: 'T', agent: 'claude', state: 'review', created_at: 0, updated_at: 0, body: {}, summary, ...more });
const labels = (m: readonly MenuEntry[]): string[] => m.flatMap((e) => (e.kind === undefined || e.kind === 'item' ? [e.label] : []));

test('the_row_menu_follows_its_row_after_an_action_without_a_reload', () => {
  const row = signal<unknown>(raw({ status: 'review', updatedAt: 0 }));
  const env = {
    menuFor: () => key,
    keys: false,
    shell: { t: (k: string) => k.replace('shell-list.menu.', '') },
    client: { entities: { row: () => () => row(), peek: () => untracked(() => row()) } },
  };
  createRoot((dispose) => {
    const menu = useDerived(() => menuFor(env as never, []));
    expect(labels(menu())).toContain('save');
    row(raw({ status: 'review', updatedAt: 0, forLater: { at: 5, by: 'you' } }));
    expect(labels(menu())).toContain('unsave');
    row(raw({ status: 'review', updatedAt: 0 }, { archived: true }));
    expect(labels(menu())).toContain('unarchive');
    expect(labels(menu())).not.toContain('save');
    dispose();
  });
});
