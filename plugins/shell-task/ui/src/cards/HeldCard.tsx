// A turn held before it starts (v1 features/hold.js, limit-hold.js): near the limit, a changed harness, project
// trust, a failed setup. It says what holds it and why; Start now (Trust and start for a changed harness or the
// repo's programs) runs it anyway, Skip this run drops it. Project trust lists what the repo would run, each with
// Open file, and offers Run without them (v1 projectCard; audit D2).
import { For, Show, type JSX } from 'solid-js';
import { Banner, Button, Inline, Stack, Text } from '@wisp/ui';
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
  return (
    <Banner
      part="turn.held"
      tone="warn"
      icon={props.held.running ? 'clock' : props.held.items ? 'warn' : 'clock'}
      title={props.held.title}
      action={
        <Inline gap="2">
          <Button size="sm" variant="ghost" disabled={!!props.held.running} onClick={props.onSkip}>
            {props.t('shell-task.held.skip')}
          </Button>
          <Show when={props.held.without && props.onWithout}>
            <Button size="sm" variant="ghost" disabled={!!props.held.running} title={props.t('shell-task.held.withoutTip')} onClick={() => props.onWithout?.()}>
              {props.held.without}
            </Button>
          </Show>
          <Button size="sm" disabled={!!props.held.running} onClick={props.onStart}>
            {props.held.start}
          </Button>
        </Inline>
      }
    >
      <Stack gap="2">
        <span>{props.held.why}</span>
        <Show when={props.held.items?.length}>
          <Stack gap="1" part="turn.held.items">
            <For each={props.held.items}>
              {(i) => (
                <Stack gap="0" part="turn.held.item">
                  <Inline gap="2">
                    <Text size="sm" tone="ink-2">
                      {i.kind}
                    </Text>
                    <Text size="sm" weight="semibold">
                      {i.name}
                    </Text>
                    <Show when={i.file && props.onOpen}>
                      <Button size="xs" variant="ghost" icon="file" title={i.file ?? ''} onClick={() => props.onOpen?.(i.file ?? '')}>
                        {props.t('shell-task.held.openFile')}
                      </Button>
                    </Show>
                  </Inline>
                  <Text size="sm" mono>
                    {i.command}
                  </Text>
                </Stack>
              )}
            </For>
          </Stack>
        </Show>
      </Stack>
    </Banner>
  );
}
