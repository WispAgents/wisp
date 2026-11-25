// Continue on… (ADR-0120): one dialog to carry a task over to another harness. The shared runner picker (the same one
// as New task and the reply box) opens on it at once unless a harness was offered (a refused or limited turn's), then
// the preview of what carries over, built by the hand-off's own note on the runtime (`runtime.handoffPreview`): the
// latest summary, the files, the turns it missed, the user's rules and each choice's token estimate. Summary only is
// the default; Include full transcript writes out every turn it missed. Continue sends one follow-up on that harness
// and model (the runtime switches the task with the turn and marks it for the Timeline's divider); Cancel changes
// nothing.
import { createResource, createSignal, For, Show, type JSX } from 'solid-js';
import { defineCommand, ownerOf, type EntityKey, type PluginClient } from '@wisp/client';
import { RunnerPicker, type Runner } from '@wisp/shell';
import { Button, Inline, Stack, Tabs, Text, TextArea, useSignal } from '@wisp/ui';
import { useShell } from '../shell.ts';
import { CARRY_ON, CONTINUE_VIEW, continueArgs, previewModel, type ContinueParams, type HandoffChoiceId, type HandoffPreview } from './model.ts';

const bare = (k: string): string => k.slice(k.indexOf(':') + 1);

const preview = defineCommand<{ key: EntityKey; agent: string }, HandoffPreview>({
  id: 'runtime.handoffPreview',
  owner: (a) => a.key,
  wire: (a) => ({ id: bare(a.key), agent: a.agent }),
  optimistic: 'none',
  pending: 'what carries over',
});
/** the hand-off itself: a follow-up on that harness, carrying what was picked (the refused and limit cards send it too) */
export const handOff = defineCommand<{ key: EntityKey; text: string; run: { agent: string; model?: string }; handoff: HandoffChoiceId }>({
  id: 'runtime.followUp',
  owner: (a) => a.key,
  wire: (a) => ({ id: bare(a.key), text: a.text, run: { agent: a.run.agent, ...(a.run.model && { model: a.run.model }) }, handoff: a.handoff }),
  optimistic: 'none',
  pending: 'Continue on…',
});

export default function ContinueOn(props: { readonly client: PluginClient; readonly params: ContinueParams }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const t = shell.t;
  const key = props.params.key;
  const session = useSignal(c.entities.row<{ agent?: string; body?: { agent?: string } }>('session', key));
  const from = (): string => session()?.agent ?? session()?.body?.agent ?? '';
  const harnesses = useSignal(shell.harnesses());
  const nameOf = (id: string): string => harnesses().find((h) => h.id === id)?.name ?? id;
  const logoOf = (id: string): string | undefined => harnesses().find((h) => h.id === id)?.logo;
  const [to, setTo] = createSignal<Runner>({ agent: props.params.agent ?? '', model: props.params.model ?? '', effort: '', account: '' });
  // the picker opens with the dialog unless a harness was offered
  const [picking, setPicking] = createSignal(!props.params.agent);
  const [choice, setChoice] = createSignal<HandoffChoiceId>('summary');
  const [text, setText] = createSignal(props.params.text ?? '');
  let pill: HTMLButtonElement | undefined;
  const [answer] = createResource(
    () => (to().agent && to().agent !== from() ? to().agent : undefined),
    (agent) => c.run(preview, { key, agent }).catch(() => undefined),
  );
  const m = (): ReturnType<typeof previewModel> => previewModel(answer.latest, choice(), t);
  const close = (): void => c.navigate({ t: 'closeOverlay', id: CONTINUE_VIEW as never });
  const same = (): boolean => to().agent !== '' && to().agent === from();
  const go = (): void => {
    const r = to();
    if (!r.agent || same()) return;
    close();
    // a refusal comes back as the shell's toast; the task's turn shows at once when it lands
    void c.run(handOff, continueArgs(key, r, choice(), text())).catch(() => undefined);
  };
  return (
    <Stack gap="4" part="continue.sheet">
      <Inline gap="2" align="center">
        <Text size="sm" tone="muted">
          {t('shell-task.continue.from', { name: nameOf(from()) })}
        </Text>
        <Button
          ref={(el) => (pill = el)}
          size="sm"
          variant="chip"
          part="continue.pick"
          haspopup="dialog"
          expanded={picking()}
          {...(to().agent && logoOf(to().agent) && { logo: logoOf(to().agent) })}
          iconEnd="chevron"
          onClick={() => setPicking(!picking())}
        >
          {to().agent ? `${nameOf(to().agent)}${to().modelLabel || to().model ? ` · ${to().modelLabel || to().model}` : ''}` : t('shell-task.continue.pick')}
        </Button>
      </Inline>
      <RunnerPicker open={picking()} onOpenChange={setPicking} anchor={() => pill} peer={ownerOf(key)} value={to()} onChange={setTo} efforts={false} />
      <Show when={same()}>
        <Text size="sm" tone="muted" part="continue.same">
          {t('shell-task.continue.same', { name: nameOf(from()) })}
        </Text>
      </Show>
      <Show when={to().agent && !same()}>
        <Stack gap="3" part="continue.preview">
          <Tabs
            variant="seg"
            part="continue.choice"
            aria-label={t('shell-task.continue.carry')}
            value={choice()}
            onChange={(v) => setChoice(v as HandoffChoiceId)}
            items={m().options.map((o) => ({ value: o.id, label: o.label, ...(o.hint && { title: o.hint }) }))}
          />
          <Show when={answer.latest} fallback={<Text size="sm" tone="muted">{t('shell-task.continue.reading')}</Text>}>
            <Show when={!m().nothing} fallback={<Text size="sm" tone="muted">{t('shell-task.continue.nothing', { name: nameOf(to().agent) })}</Text>}>
              <Stack gap="2" part="continue.rows">
                <For each={m().rows}>
                  {(r) => (
                    <Stack gap="0" part="continue.row">
                      <Text size="sm" weight="semibold">
                        {r.label}
                      </Text>
                      <Text size="sm" tone="muted">
                        {r.text}
                      </Text>
                    </Stack>
                  )}
                </For>
                <Text size="sm" part="continue.tokens">
                  {t('shell-task.continue.size', { tokens: m().tokens })}
                </Text>
              </Stack>
            </Show>
          </Show>
          <TextArea part="continue.text" minRows={2} maxRows={6} placeholder={CARRY_ON} aria-label={t('shell-task.continue.say')} value={text()} onInput={setText} />
        </Stack>
      </Show>
      <Inline gap="2" justify="end">
        <Button size="sm" variant="secondary" part="continue.cancel" onClick={close}>
          {t('shell-task.continue.cancel')}
        </Button>
        <Button size="sm" variant="primary" part="continue.go" disabled={!to().agent || same()} onClick={go}>
          {to().agent ? t('shell-task.continue.go', { name: nameOf(to().agent) }) : t('shell-task.continue.goNone')}
        </Button>
      </Inline>
    </Stack>
  );
}
