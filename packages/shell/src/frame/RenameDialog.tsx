// Rename a task in a dialog (the phone's task menu, and the desktop head's ⋯ since QA round 14 desktop #3: the in-place
// edit it started lost the caret to the menu handing focus back to ⋯): its title to edit, Enter or Save keeps it.
import { createSignal, type JSX } from 'solid-js';
import { Button, Dialog, Inline, Input } from '@wisp/ui';
import { useShell } from '../context.tsx';
import '../i18n/lazy.ts';

export function RenameDialog(props: { readonly title: string; readonly onClose: () => void; readonly onRename: (title: string) => void; readonly part: 'phone.task.rename' | 'task.rename' }): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const [value, setValue] = createSignal(props.title);
  const save = (): void => {
    const next = value().replace(/\s+/g, ' ').trim();
    props.onClose();
    if (next && next !== props.title) props.onRename(next);
  };
  return (
    <Dialog
      open
      size="sm"
      title={t('shell.more.rename')}
      onOpenChange={(open) => !open && props.onClose()}
      part={props.part}
      t={(k) => t(`kit.${k}`)}
      footer={
        <Inline gap="4" justify="end">
          <Button variant="ghost" onClick={() => props.onClose()}>
            {t('shell.stop.cancel')}
          </Button>
          <Button variant="primary" part={`${props.part}.save` as never} onClick={save}>
            {t('shell.phone.save')}
          </Button>
        </Inline>
      }
    >
      <Input
        value={value()}
        aria-label={t('shell.more.rename')}
        onInput={setValue}
        onKeyDown={(e: KeyboardEvent) => {
          if (e.key === 'Enter' && !e.isComposing) {
            e.preventDefault();
            save();
          }
        }}
      />
    </Dialog>
  );
}

