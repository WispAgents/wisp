// The saved line under the task's last answer (OfferFoot): what Save to repo wrote, and Push, so the files don't stay
// on this computer (`history.push`: onto the task's branch, or as a PR); then where it went, and Open PR.
import { Show, type JSX } from 'solid-js';
import type { EntityKey } from '@wisp/client';
import { useShell } from '@wisp/shell';
import type { ExtensionProps } from '@wisp/shell/contract';
import { Button, Icon, Text, useSignal } from '@wisp/ui';
import { t } from './i18n.ts';
import './i18n/lazy.ts';
import { pushCmd, pushedLink, pushedText, pushStates, runPush, type PushState } from './save.ts';

export interface SavedView {
  readonly files: number;
  readonly scrubbed: number;
}

/** `earlier`: what the task's slot says the last push did (a save from before this window); this window's push wins. */
export default function SavedLine(props: {
  readonly view: SavedView;
  readonly taskKey: EntityKey;
  readonly client: ExtensionProps['client'];
  readonly earlier?: PushState | undefined;
}): JSX.Element {
  const shell = useShell();
  const pushes = useSignal(pushStates);
  const state = (): PushState | undefined => pushes()[props.taskKey] ?? props.earlier;
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
        <Show when={pushedLink(state())}>
          {(link) => (
            <Button size="sm" variant="ghost" icon="external" part="history.offer.openPr" onClick={() => shell.platform.openExternal(link().url)}>
              {link().label}
            </Button>
          )}
        </Show>
      </Show>
    </>
  );
}
