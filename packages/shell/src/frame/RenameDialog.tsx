// Rename a task: one dialog, from the task head's ⋯ (and M), a row's menu and the phone's menu alike (QA round 15 #2:
// the head and the row had two). v1 actions.js rename's rules: the title selected to type over, Enter keeps it,
// Escape leaves it as it was, an empty or unchanged title sends nothing, so Save waits for a change.
import { createSignal, onMount, type JSX } from 'solid-js';
import type { EntityKey } from '@wisp/client';
import { Button, Inline, Input, Stack, useSignal } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { SESSION, type SessionRow } from '../rows.ts';
import { rename } from './commands.ts';
import '../i18n/lazy.ts';

/** The rename dialog's overlay id: `{ id }` is the task's key. */
export const RENAME = 'shell.rename';

export default function RenameDialog(props: { readonly params: { readonly id: string } }): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const key = props.params.id as EntityKey;
  const row = useSignal(shell.client.entities.row<SessionRow>(SESSION, key));
  const was = row()?.title ?? '';
  const [text, setText] = createSignal(was);
  let field: HTMLInputElement | undefined;
  const next = (): string => text().replace(/\s+/g, ' ').trim();
  const changed = (): boolean => next() !== '' && next() !== was;
  const close = (): void => shell.navigate({ t: 'closeOverlay', id: RENAME as never });
  const save = (): void => {
    if (!changed()) return;
    void shell.client.commands.run(rename, { id: key, title: next() }).catch(() => undefined);
    close();
  };
  onMount(() =>
    queueMicrotask(() => {
      field?.focus();
      field?.select();
    }),
  );
  return (
    <Stack gap="4" part="rename.body">
      <Input
        ref={(el) => (field = el)}
        value={text()}
        onInput={setText}
        aria-label={t('shell.rename.label')}
        maxLength={200}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && !e.isComposing) {
            e.preventDefault();
            save();
          }
        }}
      />
      <Inline gap="3" justify="end">
        <Button variant="ghost" onClick={close} part="list.rename.cancel">
          {t('shell.stop.cancel')}
        </Button>
        <Button variant="primary" onClick={save} disabled={!changed()} part="list.rename.save">
          {t('shell.phone.save')}
        </Button>
      </Inline>
    </Stack>
  );
}
