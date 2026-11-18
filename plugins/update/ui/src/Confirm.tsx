// Install now? The update's confirm (Settings → Updates and the top bar's pill ask the same). Its own module: the
// pill is on the first screen, and importing it from the Updates pane brought the pane, its Settings controls and
// their stylesheet with it (perf r1 #1/#2).
import type { JSX } from 'solid-js';
import { Button, Dialog, Text } from '@wisp/ui';
import { t } from './i18n.ts';
import type { UpdateState } from './types.ts';
import './i18n/lazy.ts';

type Row = Pick<UpdateState, 'latest' | 'running'>;

export function Confirm(props: { readonly open: boolean; readonly row: Row; readonly onClose: () => void; readonly onConfirm: () => void }): JSX.Element {
  return (
    <Dialog
      part="settings.update.confirm"
      open={props.open}
      onOpenChange={(o) => !o && props.onClose()}
      role="alertdialog"
      size="sm"
      title={t('update.confirm.title', { latest: props.row.latest ?? '' })}
      footer={
        <>
          <Button variant="ghost" onClick={() => props.onClose()}>
            {t('update.cancel')}
          </Button>
          <Button variant="primary" icon="update" onClick={() => props.onConfirm()}>
            {t('update.confirm.ok')}
          </Button>
        </>
      }
    >
      <Text as="p">{t('update.confirm.body', { count: props.row.running ?? 0 })}</Text>
    </Dialog>
  );
}
