// A task's PR where its text names it (ADR-0145, `shell.task:link`): the link a person pasted in a prompt, or the
// agent wrote in its answer, reads as the PR's live chip. Hovering shows its card (title, state, head → base, checks);
// a click opens the PR pane beside the task. It shows only for the task's own PRs: any other link stays a link.
import { Show, type JSX } from 'solid-js';
import type { EntityKey } from '@wisp/client';
import type { ExtensionProps } from '@wisp/shell/contract';
import { PrPeek } from '@wisp/ui';
import { t } from './i18n.ts';
import { prChipLabel, prLine, prTone } from './model/pr.ts';
import { prOfLink } from './model/link.ts';
import { TONE } from './tones.ts';
import { useSession, useShip } from './use.ts';
import './i18n/lazy.ts';

export default function PrLink(props: ExtensionProps): JSX.Element {
  const key = (): EntityKey | undefined => (props.id ?? props.session?.()?.id) as EntityKey | undefined;
  const s = useSession(props.client, key);
  const k = key();
  const ship = k ? useShip(props.client, k) : undefined; // a task's turns never change task under them
  const found = (): ReturnType<typeof prOfLink> => prOfLink(s(), props.href);
  // the task's own PR is the one wisp fixes conflicts on: its fix's stage colours it as the pill does
  const fix = (url: string) => (s()?.pr?.url === url ? s()?.conflictFix : undefined);
  return (
    <Show when={found()}>
      {(x) => (
        <PrPeek
          part="pr.link"
          label={prChipLabel(x())}
          tone={TONE[prTone(x().pr, fix(x().pr.url))]}
          live={x().pr.state === 'OPEN' && (!!x().pr.checks?.pending || !!x().pr.waitingChecks)}
          title={x().pr.title || t('wisp.ship.link.untitled', { n: x().pr.number })}
          line={prLine(t, x().pr, fix(x().pr.url))}
          {...(x().pr.branch && { head: x().pr.branch })}
          {...(x().pr.base && { base: x().pr.base })}
          checks={x().pr.checks}
          hint={t('wisp.ship.link.hint')}
          onOpen={() => ship?.openPane()}
        />
      )}
    </Show>
  );
}
