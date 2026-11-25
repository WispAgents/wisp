// `shell-task.term-screen`: a terminal task's CLI (v1 services/tui.js, term.js tuiShow) is the task's own content, the
// body edge to edge under its head (`shell.task:screen`, in place of the turns and the reply box: v1 .tui-term). Shown
// only for a terminal task (its `if`). The Terminal pane it used to open beside the task closes, so one screen streams.
import { Show, onMount, type JSX } from 'solid-js';
import type { EntityKey } from '@wisp/client';
import type { ExtensionProps } from '@wisp/shell/contract';
import { useDerived } from '@wisp/ui';
import { TERM_VIEW, keyOf } from '../open-task.ts';
import { useShell } from '../shell.ts';
import { ShellPane } from '../side/ShellPane.tsx';
import { TaskProvider, createTaskContext } from '../task-context.tsx';

export default function TermScreen(props: ExtensionProps): JSX.Element {
  const shell = useShell();
  // useDerived (round 324): a bare id is keyed by the home peer from the peer list, an alien signal
  const key = useDerived((): EntityKey | undefined => {
    const id = props.id ?? props.session?.()?.id;
    return id ? (keyOf(shell.client, id) as EntityKey | undefined) : undefined;
  });
  onMount(() => {
    const side = shell.client.nav.nav().side;
    if (side.kind === 'view' && side.id === TERM_VIEW) shell.navigate({ t: 'closeSide' });
  });
  return (
    <Show when={key()} keyed>
      {(k) => (
        <TaskProvider value={createTaskContext(props.client, k)}>
          <ShellPane kind="takeover" />
        </TaskProvider>
      )}
    </Show>
  );
}
