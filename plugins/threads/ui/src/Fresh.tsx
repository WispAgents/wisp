// Fresh thread (v1 freshThread): a dialog (a sheet on a phone) with the brief the new conversation starts from
// (threads.freshBrief: the Notes and the work as it stands), which you can edit; Start sends it (threads.fresh). The
// task, its files and its history stay. Start closes at once; a refusal opens it again with your edits (the client
// toasts why).
import { Show, createEffect, createSignal, on, type JSX } from 'solid-js';
import type { EntityKey, PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { Button, Dialog, Skeleton, Stack, Text, TextArea } from '@wisp/ui';
import { fresh, freshBrief } from './commands.ts';
import type { Key, Vars } from './i18n.ts';
import './i18n/lazy.ts';

type T = (key: Key, vars?: Vars) => string;

export interface FreshDialogProps {
  readonly client: PluginClient;
  readonly task: EntityKey;
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly t: T;
}

/** The brief, once it's here (three lines of skeleton until then: no empty box that jumps). */
function BriefBox(props: { readonly brief: string | undefined; readonly onInput: (v: string) => void; readonly t: T }): JSX.Element {
  return (
    <Show
      when={props.brief !== undefined}
      fallback={
        <Stack gap="2" part="threads.fresh.loading">
          <Skeleton width="full" />
          <Skeleton width="2/3" />
          <Skeleton width="full" />
        </Stack>
      }
    >
      <Stack gap="2">
        <Text size="sm" weight="semibold" part="threads.fresh.label">
          {props.t('threads.fresh.brief')}
        </Text>
        <TextArea part="threads.fresh.brief" value={props.brief ?? ''} onInput={props.onInput} mono autosize minRows={8} maxRows={18} aria-label={props.t('threads.fresh.brief')} spellcheck={false} />
      </Stack>
    </Show>
  );
}

export function FreshDialog(props: FreshDialogProps): JSX.Element {
  const shell = useShell();
  // the brief as you left it: kept across a close, so a refused start comes back with your edits
  const [brief, setBrief] = createSignal<string | undefined>();
  createEffect(
    on(
      () => props.open,
      (open) => {
        if (!open || brief() !== undefined) return;
        props.client.run(freshBrief, { key: props.task }).then(
          (b) => setBrief(b.text),
          () => props.onOpenChange(false),
        );
      },
    ),
  );
  const start = (): void => {
    const text = brief()?.trim();
    if (!text) return;
    props.onOpenChange(false);
    props.client.run(fresh, { key: props.task, brief: text }).then(
      (r) => {
        setBrief(undefined);
        shell.toast.success(props.t(r.queued ? 'threads.fresh.queued' : 'threads.fresh.started'));
      },
      () => props.onOpenChange(true),
    );
  };
  return (
    <Dialog
      part="threads.freshDialog"
      title={props.t('threads.fresh.title')}
      description={props.t('threads.fresh.hint')}
      open={props.open}
      onOpenChange={props.onOpenChange}
      size="md"
      footer={
        <>
          <Button part="threads.fresh.cancel" variant="ghost" onClick={() => props.onOpenChange(false)}>
            {props.t('threads.cancel')}
          </Button>
          <Button part="threads.fresh.start" variant="primary" icon="replay" disabled={!brief()?.trim()} onClick={start}>
            {props.t('threads.fresh.start')}
          </Button>
        </>
      }
    >
      <BriefBox brief={brief()} onInput={setBrief} t={props.t} />
    </Dialog>
  );
}
