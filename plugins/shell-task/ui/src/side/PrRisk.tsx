// The PR pane's merge risk section (wisp.risk, ADR-0140): the picked PR's level and score, the judge's line and its
// reasons, who judged, and Judge again. Hidden while Settings → Merge risk is off or the PR isn't open. Judge again
// says Judging at once (its pending state, then the row's running verdict) and the verdict replaces it when it lands.
import { For, Show, createMemo, createSignal, type JSX } from 'solid-js';
import { defineCommand, type EntityKey } from '@wisp/client';
import { Banner, Button, PaneHead, Stack, Text, useSignal } from '@wisp/ui';
import { useShell } from '../shell.ts';
import { useTask } from '../task-context.tsx';
import { RISK_TONE, riskSection, type RiskState } from './risk.ts';
import '../i18n/lazy.ts';

type J = { readonly key: EntityKey; readonly url: string };
const judge = defineCommand<J, { started: number; why: string }>({
  id: 'risk.judge',
  owner: (a) => a.key,
  wire: (a) => ({ id: a.key.slice(a.key.indexOf(':') + 1), force: true, url: a.url }),
  optimistic: 'none',
  pending: 'the Judge again button',
});

export function PrRisk(props: { readonly url: string; readonly open: boolean }): JSX.Element {
  const shell = useShell();
  const task = useTask();
  const t = shell.t;
  const agents = useSignal(task.client.entities.list<{ id: string; name: string }>('agent'));
  const sec = createMemo(() => riskSection(task.task(), props.url, props.open));
  // Judge again shows Judging until the judge answers the command (by then the row says running, or why not)
  const [asked, setAsked] = createSignal('');
  const state = (): Exclude<RiskState, 'off'> => {
    const st = sec().state;
    if (asked() === props.url) return 'running';
    return st === 'off' ? 'none' : st;
  };
  const by = (): string => {
    const id = sec().verdict?.by?.agent ?? '';
    return agents().find((a) => a.id === id)?.name ?? id;
  };
  const again = (): void => {
    setAsked(props.url);
    void task.client.run(judge, { key: task.key, url: props.url }).then(
      (r) => {
        setAsked('');
        if (!r.started && r.why) shell.toast.show(r.why);
      },
      () => setAsked(''), // the shell's onError toasts it
    );
  };
  const meta = (): string => {
    const v = sec().verdict;
    return v?.score !== undefined && v.current && v.status === 'done' ? `${v.score}/100` : '';
  };
  return (
    <Show when={sec().state !== 'off'}>
      <Stack gap="1" part="side.pr.risk">
        <PaneHead title={t('shell-task.pr.risk')} {...(meta() && { meta: meta() })} />
        <Banner
          part={`side.pr.risk.${state()}`}
          tone={RISK_TONE[state()]}
          title={t(`shell-task.pr.risk.${state()}`, { why: sec().verdict?.why ?? '' })}
          {...(state() !== 'running' && {
            action: (
              <Button size="sm" variant="ghost" icon="judge" onClick={again} part="side.pr.risk.again">
                {t(state() === 'none' ? 'shell-task.pr.risk.judge' : 'shell-task.pr.risk.again')}
              </Button>
            ),
          })}
        />
        <Show when={state() !== 'running' && state() !== 'none' && sec().verdict?.summary}>
          {(line) => (
            <Text as="p" size="sm">
              {line()}
            </Text>
          )}
        </Show>
        <Show when={state() !== 'running' && (sec().verdict?.reasons ?? []).length > 0}>
          <Stack gap="1" as="ul" part="side.pr.risk.reasons">
            <For each={sec().verdict?.reasons}>
              {(r) => (
                <Stack as="li">
                  <Text size="sm" tone="muted">
                    {r}
                  </Text>
                </Stack>
              )}
            </For>
          </Stack>
        </Show>
        <Show when={state() !== 'running' && sec().verdict?.by}>
          <Text size="xs" tone="muted" part="side.pr.risk.by">
            {[t('shell-task.pr.risk.by', { name: by() }), sec().verdict?.partial ? t('shell-task.pr.risk.partial') : ''].filter(Boolean).join(' · ')}
          </Text>
        </Show>
      </Stack>
    </Show>
  );
}
