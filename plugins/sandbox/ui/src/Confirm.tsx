// Delete a sandbox, asked first (UI QA r1): it can't be undone and what wasn't brought back from it goes with it, so
// the trash names the sandbox and whose it is, and only Delete drops it. Cancel, Escape, × or a click outside answer
// nothing.
import { Show, type JSX } from 'solid-js';
import { Button, Dialog, Inline, Text } from '@wisp/ui';
import { t } from './i18n.ts';
import './i18n/lazy.ts';

export interface Ask {
  readonly name: string;
  /** whose it is, in words (`ownerWords`) */
  readonly owner: string;
  readonly run: () => void;
}

export function Confirm(props: { readonly ask: Ask | undefined; readonly onClose: () => void }): JSX.Element {
  return (
    <Show when={props.ask}>
      {(a) => (
        <Dialog
          open
          role="alertdialog"
          size="sm"
          part="sandbox.confirm"
          title={t('sandbox.confirm.title', { name: a().name })}
          onOpenChange={(open) => {
            if (!open) props.onClose();
          }}
          footer={
            <Inline gap="3" justify="end">
              <Button variant="ghost" onClick={() => props.onClose()} part="sandbox.confirm.cancel">
                {t('sandbox.confirm.cancel')}
              </Button>
              <Button
                variant="danger"
                part="sandbox.confirm.ok"
                onClick={() => {
                  const run = a().run;
                  props.onClose();
                  run();
                }}
              >
                {t('sandbox.confirm.ok')}
              </Button>
            </Inline>
          }
        >
          <Text as="p">{t('sandbox.confirm.body', { owner: a().owner })}</Text>
        </Dialog>
      )}
    </Show>
  );
}
