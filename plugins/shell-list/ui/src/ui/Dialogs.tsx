// Delete task, asked over the list: v1's words, the question, then what happens, then the action on the button
// (design/copy.md). Rename is the shell's one Rename task dialog (`shell.rename`, QA round 15 #2).

import { Show, type JSX } from 'solid-js';
import { Button, Dialog, Inline } from '@wisp/ui';
import { deleteTask } from '../actions.ts';
import { KIND, type Session } from '../types.ts';
import { useEnv } from './env.ts';
import '../i18n/lazy.ts';

export function DeleteDialog(): JSX.Element {
  const env = useEnv();
  const t = env.shell.t;
  const close = (): void => env.askDelete(undefined);
  const confirm = (): void => {
    const k = env.deleting();
    close();
    if (k) deleteTask(env.actions, k);
  };
  return (
    <Show when={env.deleting()}>
      <Dialog
        open
        size="sm"
        role="alertdialog"
        title={t('shell-list.delete.title')}
        description={t('shell-list.delete.body')}
        onOpenChange={(o) => !o && close()}
        part="list.delete"
        footer={
          <Inline gap="3" justify="end">
            <Button variant="ghost" onClick={close}>
              {t('shell-list.dialog.cancel')}
            </Button>
            <Button variant="danger" onClick={confirm}>
              {t('shell-list.delete.ok')}
            </Button>
          </Inline>
        }
      />
    </Show>
  );
}
