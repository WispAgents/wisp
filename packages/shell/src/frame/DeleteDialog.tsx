// Delete task, asked first (audit A #4: the head's ⋯, ⌫ and Delete deleted at once; v1 actions.js:345 and keys.js:109
// asked on every path): the list row's words, the question, then what happens, then the action on the button. One
// dialog for the open task and the picked row; only its Delete deletes. A picked row that goes stops being picked, so
// Enter can't open it (audit A #11).
import type { JSX } from 'solid-js';
import type { EntityKey } from '@wisp/client';
import { Button, Inline, Stack, Text, useSignal } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { remove } from './commands.ts';
import { SESSION, type SessionRow } from '../rows.ts';
import { deleteWords } from './delete-words.ts';
import '../i18n/lazy.ts';

/** The delete dialog's overlay id: `{ id }` is the task's key. */
export const DELETE = 'shell.delete';

export default function DeleteDialog(props: { readonly params: { readonly id: string } }): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const key = props.params.id as EntityKey;
  const row = useSignal(shell.client.entities.row<SessionRow>(SESSION, key));
  const close = (): void => shell.navigate({ t: 'closeOverlay', id: DELETE as never });
  const confirm = (): void => {
    close();
    void shell.client.commands.run(remove, { id: key }).catch(() => undefined);
    if (shell.selection.get() === key) shell.selection.set(undefined);
    shell.navigate({ t: 'delete', id: key as never });
  };
  return (
    <Stack gap="4" part="task.delete">
      <Text tone="muted">{t(deleteWords(row()?.summary as { factory?: unknown } | undefined))}</Text>
      <Inline gap="3" justify="end">
        <Button variant="ghost" onClick={close} part="task.delete.cancel">
          {t('shell.stop.cancel')}
        </Button>
        <Button variant="danger" onClick={confirm} part="task.delete.ok">
          {t('shell.delete.ok')}
        </Button>
      </Inline>
    </Stack>
  );
}
