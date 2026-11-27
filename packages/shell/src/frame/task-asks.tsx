// The open task's two questions before it acts (TaskView.tsx): Stop (v1 #781: what else stops, its running subagents
// and a subagent's report, and that queued follow-ups stay) and Restore to before a turn (orch #17: it undoes every
// change made in the folder since then, with Undo after).
import { Show, createSignal, type JSX } from 'solid-js';
import { Button, Dialog, Inline } from '@wisp/ui';
import { stopQuestion, type StopQuestion } from './stop-question.ts';
import { stopFactoryRun, stopRun, restore } from './commands.ts';
import type { EntityKey } from '@wisp/client';
import type { useShell } from '../context.tsx';
import type { SessionRow } from '../rows.ts';

type Shell = ReturnType<typeof useShell>;

/** What the questions read of the task, and the Changes figure they read again after a restore. */
export interface TaskAsksCtx {
  readonly shell: Shell;
  readonly t: Shell['t'];
  readonly key: EntityKey;
  /** the task's own id, the part of the key after `<peer>:` */
  readonly bare: string;
  readonly run: Shell['client']['commands']['run'];
  readonly row: () => SessionRow | undefined;
  readonly sessions: () => readonly SessionRow[];
  readonly harnessView: (id: string) => { name: string; logo?: string };
  readonly refetchChanges: () => unknown;
}

export function useTaskAsks(c: TaskAsksCtx) {
  const { shell, t, key, bare, run, row, sessions, harnessView, refetchChanges } = c;
// Stop asks first (v1 #781): what else stops (its running subagents, a subagent's report), and that queued follow-ups stay
const [stopAsk, setStopAsk] = createSignal<(StopQuestion & { readonly factory?: true }) | undefined>();
const [stopping, setStopping] = createSignal(false);
const askStop = (): void => {
  const r = row();
  if (!r) return;
  const live = sessions().filter((x) => (x.parent_id === key || x.parent_id === bare) && x.state === 'running').length;
  setStopAsk(stopQuestion({ sub: !!r.parent_id, name: harnessView(r.agent).name, live, queued: Number(r.summary.queued ?? 0) }, t));
};
// Restore to before a turn asks first (orch #17): it undoes every change made in the folder since that turn
const [restoreAsk, setRestoreAsk] = createSignal<string | undefined>();
const doRestore = (): void => {
  const turnId = restoreAsk();
  setRestoreAsk(undefined);
  if (!turnId) return;
  setRestoring(true);
  void run(restore, { id: key, turn: turnId })
    .then(
      // the head's Changes figure shows it; one that restored nothing says so. What the folder held just before
      // is kept: Undo puts it back (audit A tasks #1: a restore lost work with no way back)
      (r) => {
        if (r?.restored === false) return void shell.toast.show(t('shell.restore.nothing'), { tone: 'error' });
        const undo = r?.undo;
        if (!undo) return;
        shell.toast.show(t('shell.restore.done'), {
          action: { label: t('shell.restore.undo'), onClick: () => void run(restore, { id: key, turn: turnId, undo }).then(() => refetchChanges(), () => undefined) },
        });
      },
      () => undefined,
    )
    .finally(() => {
      setRestoring(false);
      // what git shows now in the folder, read again: the figure isn't left at what it was before (orch #17)
      void refetchChanges();
    });
};
const doStop = (): void => {
  const factory = stopAsk()?.factory === true;
  setStopAsk(undefined);
  // checked again after the answer: a run that ended while the question was open isn't stopped by an old Yes
  const st = row()?.state;
  if (st !== 'running' && st !== 'waiting') return;
  setStopping(true);
  void run(factory ? stopFactoryRun : stopRun, { id: key })
    .catch(() => undefined)
    .finally(() => setStopping(false));
};
const [restoring, setRestoring] = createSignal(false);
  /** the two dialogs, drawn with the page */
  const asks = (): JSX.Element => (
    <>
      <Show when={stopAsk()}>
        {(q) => (
          <Dialog
            open
            role="alertdialog"
            size="sm"
            title={q().title}
            description={q().body}
            onOpenChange={(open) => !open && setStopAsk(undefined)}
            part="task.stopAsk"
            footer={
              <Inline gap="4" justify="end">
                <Button variant="ghost" onClick={() => setStopAsk(undefined)}>
                  {t('shell.stop.cancel')}
                </Button>
                <Button variant="danger" part="task.stopAsk.ok" onClick={doStop}>
                  {q().ok}
                </Button>
              </Inline>
            }
          />
        )}
      </Show>
      <Show when={restoreAsk()}>
        <Dialog
          open
          role="alertdialog"
          size="sm"
          title={t('shell.restore.title')}
          description={t('shell.restore.body')}
          onOpenChange={(open) => !open && setRestoreAsk(undefined)}
          part="task.restoreAsk"
          footer={
            <Inline gap="4" justify="end">
              <Button variant="ghost" onClick={() => setRestoreAsk(undefined)}>
                {t('shell.stop.cancel')}
              </Button>
              <Button variant="danger" part="task.restoreAsk.ok" onClick={doRestore}>
                {t('shell.restore.ok')}
              </Button>
            </Inline>
          }
        />
      </Show>
    </>
  );
  return { askStop, setStopAsk, stopping, setRestoreAsk, restoring, asks };
}
