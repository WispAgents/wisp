// TRY-15: one pane's crash never takes the app down. A view that throws while it draws (the task page, Home, a
// plugin's view) shows what happened, Try again and Home in its own pane; the list and the other panes keep working.
// The error goes to the console and to Diagnostics, never to a server (no paths or prompts leave the machine).

import { ErrorBoundary, type JSX } from 'solid-js';
import { Button, EmptyState, Inline } from '@wisp/ui';
import { useShell } from '../context.tsx';

export function PaneBoundary(props: { readonly pane: string; readonly children: JSX.Element }): JSX.Element {
  const shell = useShell();
  return (
    <ErrorBoundary
      fallback={(err: unknown, reset) => {
        console.error(`wisp: the ${props.pane} pane failed`, err);
        shell.client.devtools.record({ kind: 'warn', message: `${props.pane} pane failed: ${String(err)}` });
        const why = err instanceof Error ? err.message : String(err);
        return (
          <EmptyState
            icon="warn"
            title={shell.t('shell.pane.failed')}
            part="shell.pane.failed"
            action={
              <Inline gap="2" justify="center">
                <Button size="sm" onClick={reset}>
                  {shell.t('shell.pane.retry')}
                </Button>
                <Button size="sm" variant="ghost" onClick={() => shell.navigate({ t: 'home' })}>
                  {shell.t('shell.home')}
                </Button>
              </Inline>
            }
          >
            {why}
          </EmptyState>
        );
      }}
    >
      {props.children}
    </ErrorBoundary>
  );
}
