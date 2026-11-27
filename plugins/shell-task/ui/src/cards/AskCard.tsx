// The agent's live question or a tool's approval over the reply box (v1 turns.js renderAskCard): Needs you,
// answered here with a choice, Allow / Always / Deny, or typed text below. The card goes at once on an answer.
import { createSignal, For, Show, type JSX } from 'solid-js';
import { AskOptions, AskPanel, Box, Button, Stack, Text, Wrap } from '@wisp/ui';
import type { Ask, SetupAsk } from '../foot/model.ts';
import '../i18n/lazy.ts';

export function AskCard(props: { readonly ask: Ask; readonly t: (k: string) => string; readonly onAnswer: (text: string) => void }): JSX.Element {
  const t = props.t;
  return (
    <Show when={props.ask.setup} fallback={<QuestionCard ask={props.ask} t={t} onAnswer={props.onAnswer} />}>
      {(s) => (
        <Box part="task.ask" pad="3" radius="md" border="line" surface="surface" aria-label={t('shell-task.ask.label')}>
          <Stack gap="2">
            <Text size="sm" tone="muted">
              {t('shell-task.ask.setup')}
            </Text>
            <SetupBody setup={s()} t={t} />
            <Wrap gap="2">
              <SetupButtons t={t} onAnswer={props.onAnswer} />
            </Wrap>
            <Text size="sm" tone="muted">
              {t('shell-task.ask.setupNote')}
            </Text>
          </Stack>
        </Box>
      )}
    </Show>
  );
}

/** v1 .ask-card: the head, the call as a headline and the agent's explanation under it (markdown, never its marks),
 *  then numbered answers that wrap whole (an approval: Allow / Always / Deny). */
function QuestionCard(props: { readonly ask: Ask; readonly t: (k: string) => string; readonly onAnswer: (text: string) => void }): JSX.Element {
  const t = props.t;
  return (
    <AskPanel
      part="task.ask"
      aria-label={t('shell-task.ask.label')}
      label={t(props.ask.approval ? 'shell-task.ask.approval' : 'shell-task.ask.needs')}
      icon={props.ask.approval ? 'warn' : 'ask'}
      question={props.ask.question}
      foot={props.ask.approval ? undefined : t('shell-task.ask.orType')}
    >
      <Show
        when={props.ask.approval}
        fallback={<AskOptions options={props.ask.options.map((label) => ({ label }))} part="task.ask.options" optionPart="task.ask.option" onPick={(i) => props.onAnswer(props.ask.options[i] ?? '')} />}
      >
        <Wrap gap="2">
          <Button size="sm" variant="primary" onClick={() => props.onAnswer('allow')}>
            {t('shell-task.ask.allow')}
          </Button>
          <Button size="sm" variant="secondary" onClick={() => props.onAnswer('always')}>
            {t('shell-task.ask.always')}
          </Button>
          <Button size="sm" variant="ghost" onClick={() => props.onAnswer('deny')}>
            {t('shell-task.ask.deny')}
          </Button>
        </Wrap>
      </Show>
    </AskPanel>
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
      {/* Show all: the same lines, none cut, then each kit file's body (audit G tasks #9: it was the JSON) */}
      <For each={all() && props.setup.full ? props.setup.full.split('\n') : props.setup.lines}>
        {(l) => (
          <Text size="sm" tone="muted" breakAnywhere part={all() ? 'task.ask.setup.full' : 'task.ask.setup.line'}>
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
      {/* offered only when a line was cut or a kit has bodies: the server sends no `full` otherwise */}
      <Show when={props.setup.full}>
        <Wrap gap="2">
          <Button size="sm" variant="ghost" part="task.ask.setup.showAll" onClick={() => setAll(!all())}>
            {props.t(all() ? 'shell-task.ask.showLess' : 'shell-task.ask.showAll')}
          </Button>
        </Wrap>
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
