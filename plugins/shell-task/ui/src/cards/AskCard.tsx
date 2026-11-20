// The agent's live question or a tool's approval over the reply box (v1 turns.js renderAskCard): Needs you,
// answered here with a choice, Allow / Always / Deny, or typed text below. The card goes at once on an answer.
import { createSignal, For, Show, type JSX } from 'solid-js';
import { Box, Button, Code, Stack, Text, Wrap } from '@wisp/ui';
import type { Ask, SetupAsk } from '../foot/model.ts';
import '../i18n/lazy.ts';

export function AskCard(props: { readonly ask: Ask; readonly t: (k: string) => string; readonly onAnswer: (text: string) => void }): JSX.Element {
  const t = props.t;
  return (
    <Box part="task.ask" pad="3" radius="md" border="line" surface="surface" aria-label={t('shell-task.ask.label')}>
      <Stack gap="2">
        <Text size="sm" tone="muted">
          {t(props.ask.setup ? 'shell-task.ask.setup' : props.ask.approval ? 'shell-task.ask.approval' : 'shell-task.ask.needs')}
        </Text>
        <Show when={props.ask.setup} fallback={<Text weight="medium">{props.ask.question}</Text>}>
          {(s) => <SetupBody setup={s()} t={t} />}
        </Show>
        <Wrap gap="2">
          <Show when={!props.ask.setup} fallback={<SetupButtons t={t} onAnswer={props.onAnswer} />}>
            <Show
              when={props.ask.approval}
              fallback={
                <For each={props.ask.options}>
                  {(o, i) => (
                    <Button size="sm" variant={i() === 0 ? 'primary' : 'secondary'} onClick={() => props.onAnswer(o)}>
                      {o}
                    </Button>
                  )}
                </For>
              }
            >
              <Button size="sm" variant="primary" onClick={() => props.onAnswer('allow')}>
                {t('shell-task.ask.allow')}
              </Button>
              <Button size="sm" variant="secondary" onClick={() => props.onAnswer('always')}>
                {t('shell-task.ask.always')}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => props.onAnswer('deny')}>
                {t('shell-task.ask.deny')}
              </Button>
            </Show>
          </Show>
        </Wrap>
        <Show when={!props.ask.approval}>
          <Text size="sm" tone="muted">
            {t(props.ask.setup ? 'shell-task.ask.setupNote' : 'shell-task.ask.orType')}
          </Text>
        </Show>
      </Stack>
    </Box>
  );
}

/** A setup card's body (ADR-0113): what would change, its other facts, then each command it would run, verbatim. */
function SetupBody(props: { readonly setup: SetupAsk; readonly t: (k: string) => string }): JSX.Element {
  const [all, setAll] = createSignal(false);
  return (
    <Stack gap="1">
      <Text weight="medium" breakAnywhere part="task.ask.setup.headline">
        {props.setup.headline}
      </Text>
      <For each={props.setup.lines}>
        {(l) => (
          <Text size="sm" tone="muted" breakAnywhere part="task.ask.setup.line">
            {l}
          </Text>
        )}
      </For>
      <Show when={props.setup.commands.length > 0}>
        <Text size="sm" tone="muted">
          {props.t('shell-task.ask.runs')}
        </Text>
        <For each={props.setup.commands}>
          {(c) => (
            <Text size="sm" mono breakAnywhere part="task.ask.setup.command">
              {c}
            </Text>
          )}
        </For>
      </Show>
      {/* everything that will be written, word for word (a kit's bodies, a brief, a shared crew's members) */}
      <Show when={props.setup.full}>
        <Wrap gap="2">
          <Button size="sm" variant="ghost" part="task.ask.setup.showAll" onClick={() => setAll(!all())}>
            {props.t(all() ? 'shell-task.ask.showLess' : 'shell-task.ask.showAll')}
          </Button>
        </Wrap>
        <Show when={all()}>
          <Code block wrap plain part="task.ask.setup.full">
            {props.setup.full}
          </Code>
        </Show>
      </Show>
    </Stack>
  );
}

/** Allow or Deny: a setup card has no Always (v1 asked each time). */
function SetupButtons(props: { readonly t: (k: string) => string; readonly onAnswer: (text: string) => void }): JSX.Element {
  return (
    <>
      <Button size="sm" variant="primary" onClick={() => props.onAnswer('Allow')}>
        {props.t('shell-task.ask.allow')}
      </Button>
      <Button size="sm" variant="ghost" onClick={() => props.onAnswer('Deny')}>
        {props.t('shell-task.ask.deny')}
      </Button>
    </>
  );
}
