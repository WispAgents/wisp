// A side discussion's proposed changes on its main task (audit user-side-edits-for-approval): a side's write to a file
// this task changed isn't made there, it waits here as a card, the change as a diff, from which side discussion, with
// Apply (written in this task's folder while the file still reads as it did) and Discard. Loaded with the first
// proposal only (TaskView's `lazy`), so the task page's first bundle doesn't carry it.
import { For, Show, type JSX } from 'solid-js';
import { Button, Card, Diff, Inline, Stack, Text } from '@wisp/ui';
import { diffFileOf } from '@wisp/ui/pure';
import type { ProposalRow } from './commands.ts';
import '../i18n/lazy.ts';

export interface ProposalsProps {
  readonly items: readonly ProposalRow[];
  readonly t: (key: string, vars?: Record<string, string>) => string;
  readonly onApply: (proposal: string) => void;
  readonly onDiscard: (proposal: string) => void;
  /** opens the side discussion it came from */
  readonly onOpenSide?: (side: string) => void;
}

export default function Proposals(props: ProposalsProps): JSX.Element {
  return (
    <Show when={props.items.length > 0}>
      <Stack gap="3" part="task.proposals">
        <For each={props.items}>
          {(p) => (
            <Card
              part="task.proposal"
              tone="accent"
              header={
                <Inline gap="2" align="center" part="task.proposal.head">
                  <Text size="sm" weight="medium">
                    {props.t('shell.proposal.head', { path: p.path })}
                  </Text>
                  <span data-kit="grow" />
                  <Show when={props.onOpenSide} fallback={<Text size="sm" tone="muted">{props.t('shell.proposal.from', { title: p.sideTitle })}</Text>}>
                    {(open) => (
                      <Button variant="ghost" size="xs" icon="discuss" onClick={() => open()(p.side)} part="task.proposal.side">
                        {props.t('shell.proposal.from', { title: p.sideTitle })}
                      </Button>
                    )}
                  </Show>
                </Inline>
              }
              footer={
                <Inline gap="2" align="center" part="task.proposal.bar">
                  <Text size="sm" tone="muted">
                    {props.t('shell.proposal.hint')}
                  </Text>
                  <span data-kit="grow" />
                  <Button variant="ghost" size="sm" onClick={() => props.onDiscard(p.id)} part="task.proposal.discard">
                    {props.t('shell.proposal.discard')}
                  </Button>
                  <Button variant="primary" size="sm" icon="merge" onClick={() => props.onApply(p.id)} part="task.proposal.apply">
                    {props.t('shell.proposal.apply')}
                  </Button>
                </Inline>
              }
            >
              <Diff part="task.proposal.diff" files={[diffFileOf(p.path, p.before ?? '', p.after)]} />
            </Card>
          )}
        </For>
      </Stack>
    </Show>
  );
}
