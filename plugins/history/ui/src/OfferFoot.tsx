// Under the task's last answer (`shell.turn:foot`, the turn's key as its id): Save to repo when the repo offers it
// (`track: ask`, the task done), then Saving… and what was saved, whichever way it was saved (the answer's button or
// the ⋯ menu). Nothing on other turns, nor on a task nobody offered or saved here.
import { Match, Show, Switch, createMemo, type JSX } from 'solid-js';
import { entityKey, ownerOf, type EntityKey } from '@wisp/client';
import type { ExtensionProps } from '@wisp/shell/contract';
import { Button, Icon, Inline, Text, useSignal } from '@wisp/ui';
import { t } from './i18n.ts';
import './i18n/lazy.ts';
import { historyOf, lastTurnOf, offerView, runSave, saveCmd, saveStates, type TaskRow } from './save.ts';

const bare = (k: string): string => k.slice(k.indexOf(':') + 1);

export default function OfferFoot(props: ExtensionProps): JSX.Element {
  const c = props.client;
  const turnKey = (): string => props.id ?? '';
  const task = (): (TaskRow & { id: string }) | undefined => props.session?.() as (TaskRow & { id: string }) | undefined;
  const taskKey = (): EntityKey => entityKey(ownerOf(turnKey() as EntityKey), bare(String(task()?.id ?? ''))) as EntityKey;
  const turns = useSignal(c.entities.list<unknown>('turn'));
  const last = createMemo(() => lastTurnOf(turns() as readonly unknown[], bare(String(task()?.id ?? ''))) === bare(turnKey()));
  // this window's saves (a client signal, bridged so the line follows Saving… → Saved)
  const states = useSignal(saveStates);
  const view = createMemo(() => offerView(historyOf(task()), states()[taskKey()]));
  const save = (): void => void runSave((key) => c.run(saveCmd, { key }), taskKey());
  return (
    <Show when={last() && view().v !== 'none'}>
      <Inline gap="2" align="center" part="history.offer">
        <Switch>
          <Match when={view().v === 'offer' || view().v === 'saving'}>
            <Icon name="branch" size="sm" tone="faint" />
            <Text size="sm" tone="muted" truncate part="history.offer.text" title={t('history.offer.tip')}>
              {t('history.offer.text')}
            </Text>
            <Button size="sm" variant="secondary" part="history.offer.save" loading={view().v === 'saving'} onClick={save}>
              {t('history.save')}
            </Button>
          </Match>
          <Match when={view().v === 'saved'}>
            {(() => {
              const v = view() as { files: number; scrubbed: number };
              return (
                <>
                  <Icon name="check" size="sm" tone="faint" />
                  <Text size="sm" tone="muted" truncate part="history.offer.saved">
                    {v.scrubbed > 0 ? t('history.saved.scrubbed', { count: v.files, scrubbed: v.scrubbed }) : t('history.saved', { count: v.files })}
                  </Text>
                </>
              );
            })()}
          </Match>
        </Switch>
      </Inline>
    </Show>
  );
}
