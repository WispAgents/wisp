// A turn held before it starts (v1 features/hold.js, limit-hold.js): near the limit, a changed harness, project
// trust, a failed setup. It says what holds it and why; Start now (Trust and start for a changed harness or the
// repo's programs) runs it anyway, Skip this run drops it. Project trust lists what the repo would run, each with
// Open file, and offers Run without them (v1 projectCard; audit D2).
import { For, Show, type JSX } from 'solid-js';
import { Banner, Button, Code, Stack, Text, Wrap } from '@wisp/ui';
import type { Held, T } from './model.ts';
import '../i18n/lazy.ts';

export function HeldCard(props: {
  readonly held: Held;
  readonly t: T;
  readonly onStart: () => void;
  readonly onSkip: () => void;
  readonly onWithout?: () => void;
  /** opens an item's file in the editor; absent: no editor to open it in */
  readonly onOpen?: (file: string) => void;
}): JSX.Element {
  // v1 projectCard / heldCard: what it says, what the repo would run, then the buttons on a row of their own under them,
  // the primary first (.lm-acts). As the Banner's side action they took the card's width (audit E2/E3).
  return (
    <Banner part="turn.held" tone="warn" icon={props.held.running ? 'clock' : props.held.items ? 'warn' : 'clock'} title={props.held.title}>
      <Stack gap="2">
        <span>{props.held.why}</span>
        <Show when={props.held.items?.length}>
          <Stack gap="2" part="turn.held.items">
            <For each={props.held.items}>
              {(i) => (
                <Stack gap="1" part="turn.held.item">
                  <Wrap gap="2" align="center">
                    <Text size="sm" tone="ink-2">
                      {i.kind}
                    </Text>
                    <Text size="sm" weight="semibold">
                      {i.name}
                    </Text>
                    <Show when={i.file}>
                      <Text size="sm" tone="muted" mono part="turn.held.file">
                        {i.file}
                      </Text>
                    </Show>
                    <Show when={i.file && props.onOpen}>
                      <Button size="xs" variant="ghost" icon="external" title={i.file ?? ''} onClick={() => props.onOpen?.(i.file ?? '')}>
                        {props.t('shell-task.held.openFile')}
                      </Button>
                    </Show>
                  </Wrap>
                  <Code>{i.command}</Code>
                </Stack>
              )}
            </For>
          </Stack>
        </Show>
        <Wrap gap="2" part="turn.held.actions">
          <Button size="sm" variant="primary" icon="play" disabled={!!props.held.running} onClick={props.onStart}>
            {props.held.start}
          </Button>
          <Show when={props.held.without && props.onWithout}>
            <Button size="sm" variant="ghost" icon="skip" disabled={!!props.held.running} title={props.t('shell-task.held.withoutTip')} onClick={() => props.onWithout?.()}>
              {props.held.without}
            </Button>
          </Show>
          <Button size="sm" variant="ghost" icon="skip" disabled={!!props.held.running} onClick={props.onSkip}>
            {props.t('shell-task.held.skip')}
          </Button>
        </Wrap>
      </Stack>
    </Banner>
  );
}
