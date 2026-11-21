// @vitest-environment happy-dom
// Audit E6: Settings → Worktrees only watched the rows of the last look, so a stray made after wisp started (a leftover
// folder, a worktree to bring in) never showed however long the pane stayed open. Opening it asks for a look.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { signal } from '@wisp/client';
import { ShellProvider } from '@wisp/shell';
import { KitProvider } from '@wisp/ui';
import Worktrees from '../src/panes/Worktrees.tsx';

afterEach(() => {
  document.body.innerHTML = '';
});

test('opening_the_pane_asks_for_a_look_audit_E6', () => {
  const sent: string[] = [];
  const scope = { child: () => scope, dispose: () => undefined, interval: () => () => undefined, add: () => undefined, timeout: () => () => undefined };
  const client = {
    home: 'laptop',
    scope,
    watch: () => undefined,
    entities: { list: () => signal([]), row: () => signal(undefined) },
    run: async (cmd: { id: string }) => {
      sent.push(cmd.id);
      return {};
    },
    navigate: () => undefined,
  };
  const shell = { toast: { success: () => undefined, error: () => undefined } };
  const host = document.createElement('div');
  document.body.append(host);
  const stop = render(
    () => (
      <KitProvider t={(k) => k}>
        <ShellProvider value={shell as never}>
          <Worktrees client={client as never} />
        </ShellProvider>
      </KitProvider>
    ),
    host,
  );
  expect(sent).toContain('worktrees.view');
  stop();
});
