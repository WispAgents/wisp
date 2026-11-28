// Under the task's last answer (`shell.turn:foot`, the turn's key as its id): Save to repo when the repo offers it
// (`track: ask`, the task done: nothing while it works, a reopened one too), then Saving… and what was saved, whichever way it was saved (the answer's button or
// the ⋯ menu), then where Push sent it (after a reload too, from the task's slot). Also toasts once per save so mobile
// users tapping ⋮ → Save to repo see immediate confirmation even when scrolled away from the bottom of the turn, and
// once per ⋯ → Push to repo, naming the branch with its PR.
import { Match, Show, Switch, createEffect, createMemo, type JSX } from 'solid-js';
import { entityKey, ownerOf, type EntityKey } from '@wisp/client';
import { useShell } from '@wisp/shell';
import type { ExtensionProps } from '@wisp/shell/contract';
import { Button, Icon, Inline, Text, useSignal } from '@wisp/ui';
import { t } from './i18n.ts';
import './i18n/lazy.ts';
import { historyOf, lastTurnOf, offerView, pushedLink, pushedText, pushStates, runSave, saveCmd, saveStates, working, type OfferView, type TaskRow } from './save.ts';
import SavedLine, { type SavedView } from './SavedLine.tsx';

const bare = (k: string): string => k.slice(k.indexOf(':') + 1);
const toasted = new Set<string>();

export default function OfferFoot(props: ExtensionProps): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const turnKey = (): string => props.id ?? '';
  const peer = (): string => ownerOf(turnKey() as EntityKey);
  const task = (): (TaskRow & { id: string }) | undefined => props.session?.() as (TaskRow & { id: string }) | undefined;
  const taskKey = (): EntityKey => entityKey(ownerOf(turnKey() as EntityKey), bare(String(task()?.id ?? ''))) as EntityKey;
  const turns = useSignal(c.entities.list<unknown>('turn'));
  const last = createMemo(() => lastTurnOf(turns() as readonly unknown[], bare(String(task()?.id ?? '')), peer()) === bare(turnKey()));
  // this window's saves (a client signal, bridged so the line follows Saving… → Saved)
  const states = useSignal(saveStates);
  const pushes = useSignal(pushStates);
  const view = createMemo(() => offerView(historyOf(task()), states()[taskKey()], pushes()[taskKey()]));
  const kept = (): Extract<OfferView, { v: 'kept' }> | undefined => {
    const v = view();
    return v.v === 'kept' ? v : undefined;
  };
  const save = (): void => void runSave((key) => c.run(saveCmd, { key }), taskKey());
  createEffect(() => {
    const v = view();
    if (!last() || v.v !== 'saved') return;
    const stamp = `${taskKey()}:${v.at}`;
    if (toasted.has(stamp)) return;
    toasted.add(stamp);
    const msg = v.scrubbed > 0 ? t('history.saved.scrubbed', { count: v.files, scrubbed: v.scrubbed }) : t('history.saved', { count: v.files });
    shell.toast.success(msg);
  });
  createEffect(() => {
    const p = pushes()[taskKey()];
    if (!last() || p?.s !== 'pushed' || !p.tell) return;
    const stamp = `${taskKey()}:push:${p.at ?? 0}`;
    if (toasted.has(stamp)) return;
    toasted.add(stamp);
    const link = pushedLink(p);
    shell.toast.success(pushedText(p), link ? { action: { label: link.label, onClick: () => shell.platform.openExternal(link.url) } } : undefined);
  });
  return (
    <Show when={last() && !working(task()) && view().v !== 'none'}>
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
            <SavedLine view={view() as SavedView} taskKey={taskKey()} client={c} />
          </Match>
          <Match when={kept()}>
            {(k) => <SavedLine view={{ files: k().files, scrubbed: 0 }} taskKey={taskKey()} client={c} earlier={k().pushed} />}
          </Match>
        </Switch>
      </Inline>
    </Show>
  );
}
