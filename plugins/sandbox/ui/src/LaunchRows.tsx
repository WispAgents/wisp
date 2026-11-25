// The `/sandbox` launches on top of the task list (`shell.list:top`, the owner's 2026-11-25 report): each as a task
// row from the first frame, its mark a small glowing grid while the sandbox comes up, then gone when its task's own
// row is there; one waiting for Claude's sign-in asks for it, one that failed says why until it's dismissed. A press
// opens its page.
import { For, type JSX } from 'solid-js';
import type { PeerId } from '@wisp/client';
import type { ExtensionProps } from '@wisp/shell/contract';
import { GlowGrid, IconButton, ListRow, Pill, Stack, Time, useSignal } from '@wisp/ui';
import { dismiss } from './commands.ts';
import { t } from './i18n.ts';
import { LAUNCH, shown, type LaunchRow } from './launches.ts';
import { LAUNCHING } from './views.ts';
import './i18n/lazy.ts';

const say = t as (k: string, v?: Readonly<Record<string, string | number>>) => string;

export function subOf(r: LaunchRow): string {
  if (r.state === 'signin') return t('sandbox.row.signin');
  if (r.state === 'failed') return say('sandbox.row.failed', { why: r.error ?? '' });
  return t('sandbox.row.preparing');
}

export default function LaunchRows(props: ExtensionProps): JSX.Element {
  const c = props.client;
  const all = useSignal(c.entities.list<LaunchRow>(LAUNCH));
  const rows = (): LaunchRow[] => shown(all() as readonly LaunchRow[]);
  return (
    <Stack gap="0" part="sandbox.launches">
      <For each={rows()}>
        {(r) => (
          <ListRow
            part="sandbox.launch.row"
            layout="feed"
            title={r.title}
            strong={r.state === 'signin'}
            lead={<GlowGrid size="sm" state={r.state === 'failed' ? 'failed' : r.state === 'signin' ? 'waiting' : 'working'} />}
            sub={subOf(r)}
            subTone={r.state === 'preparing' ? 'accent-ink' : 'warn'}
            subTitle={subOf(r)}
            pills={
              <Pill tone="neutral" variant="outline" icon="box">
                {t('sandbox.pill')}
              </Pill>
            }
            time={<Time at={r.at} />}
            quick={r.state === 'preparing' ? 0 : 1}
            actions={
              r.state === 'preparing' ? undefined : (
                <IconButton icon="x" size="sm" variant="ghost" label={say('sandbox.row.dismiss', { name: r.title })} onClick={() => void c.run(dismiss, { peer: c.home as PeerId, id: r.id })} />
              )
            }
            onOpen={() => c.navigate({ t: 'openView', id: LAUNCHING as never, params: { id: r.id } as never })}
          />
        )}
      </For>
    </Stack>
  );
}
