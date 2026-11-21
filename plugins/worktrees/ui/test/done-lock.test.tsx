// @vitest-environment happy-dom
// round 324: Remove its worktrees?'s "Always do this" hides when a formula or a plugin locks worktrees.onDone, and
// follows that `setting` row while the dialog is open: a row that lands (or locks) after it opened hides the box,
// one unlocked shows it. The dialog read the row directly in its JSX, which Solid doesn't follow.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { signal } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import DoneDialog from '../src/panes/DoneDialog.tsx';
import type { DoneFolder } from '../src/types.ts';
import { until } from './until.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

const folder: DoneFolder = { path: '/w/a', task: 's1', title: 'Fix', own: true, branch: 'wisp/a', size: null, only: 0, uncommitted: 0, unpushed: 0, pr: null, kept: false };

test('always_do_this_hides_when_the_setting_row_locks_after_the_dialog_opened_and_shows_when_unlocked', async () => {
  const setting = signal<{ key: string; value: unknown; locked_by?: string | null } | undefined>(undefined);
  const client = {
    home: 'laptop',
    entities: { row: (kind: string, key: string) => (kind === 'setting' && key === 'laptop:worktrees.onDone' ? setting : signal(undefined)) },
    run: async () => ({ folders: [folder], onDone: 'ask' }),
    navigate: () => undefined,
  };
  const shell = { toast: { success: () => undefined, error: () => undefined } };
  const host = document.createElement('div');
  document.body.append(host);
  const stop = render(
    () => (
      <KitProvider t={(k) => k}>
        <ShellProvider value={shell as never}>
          <DoneDialog client={client as never} params={{ key: 'laptop:s1', only: false, n: 1 } as never} />
        </ShellProvider>
      </KitProvider>
    ),
    host,
  );
  const always = (): Element | null => host.querySelector('[data-part="worktrees.folders.always"]');
  await until(() => host.querySelector('[data-part="worktrees.folders.rows"] input') !== null);
  expect(always(), 'no setting row yet: the box shows').not.toBeNull();
  setting({ key: 'worktrees.onDone', value: 'ask', locked_by: 'formula' });
  expect(always(), 'locked after it opened: the box goes').toBeNull();
  setting({ key: 'worktrees.onDone', value: 'ask', locked_by: null });
  expect(always(), 'unlocked: it shows again').not.toBeNull();
  stop();
});
