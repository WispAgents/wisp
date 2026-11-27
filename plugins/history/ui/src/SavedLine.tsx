// The saved line under the task's last answer (OfferFoot): what Save to repo wrote, and Push, so the files don't stay
// on this computer (`history.push`: onto the task's branch, or as a PR); then where it went, and Open PR.
import { Show, type JSX } from 'solid-js';
import type { EntityKey } from '@wisp/client';
import { useShell } from '@wisp/shell';
import type { ExtensionProps } from '@wisp/shell/contract';
import { Button, Icon, Text, useSignal } from '@wisp/ui';
import { t } from './i18n.ts';
import './i18n/lazy.ts';
import { pushCmd, pushStates, runPush, type PushState } from './save.ts';

export interface SavedView {
  readonly files: number;
  readonly scrubbed: number;
}

/** What the line says once pushed: the branch and its PR, or that the remote had it already. */
export function pushedText(p: PushState | undefined): string {
  if (p?.s !== 'pushed') return '';
  if (!p.branch) return t('history.pushed.none');
  return p.pr ? t('history.pushed.pr', { branch: p.branch }) : t('history.pushed', { branch: p.branch });
}

export default function SavedLine(props: { readonly view: SavedView; readonly taskKey: EntityKey; readonly client: ExtensionProps['client'] }): JSX.Element {
  const shell = useShell();
  const pushes = useSignal(pushStates);
  const state = (): PushState | undefined => pushes()[props.taskKey];
  const pr = (): string | undefined => {
    const s = state();
    return s?.s === 'pushed' ? s.pr : undefined;
  };
  const push = (): void => void runPush((key) => props.client.run(pushCmd, { key }), props.taskKey);
  const saved = (): string =>
    props.view.scrubbed > 0 ? t('history.saved.scrubbed', { count: props.view.files, scrubbed: props.view.scrubbed }) : t('history.saved', { count: props.view.files });
  return (
    <>
      <Icon name="check" size="sm" tone="faint" />
      <Show
        when={state()?.s === 'pushed'}
        fallback={
          <>
            <Text size="sm" tone="muted" truncate part="history.offer.saved">
              {saved()}
            </Text>
            <Button size="sm" variant="secondary" icon="arrow-up" part="history.offer.push" title={t('history.push.tip')} loading={state()?.s === 'pushing'} onClick={push}>
              {t('history.push')}
            </Button>
          </>
        }
      >
        <Text size="sm" tone="muted" truncate part="history.offer.pushed">
          {pushedText(state())}
        </Text>
        <Show when={pr()}>
          {(url) => (
            <Button size="sm" variant="ghost" icon="external" part="history.offer.openPr" onClick={() => shell.platform.openExternal(url())}>
              {t('history.openPr')}
            </Button>
          )}
        </Show>
      </Show>
    </>
  );
}
