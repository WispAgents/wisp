// Over a turn that switched the task to another harness (`shell.turn:head`, ADR-0125): one divider in the Timeline,
// "Switched from Claude Code (Opus) to Codex (GPT-5) · context: summary", from the turn's switch mark.
import { Show, type JSX } from 'solid-js';
import type { ExtensionProps } from '@wisp/shell/contract';
import { modelWords } from '@wisp/shell/pure';
import { Divider, useSignal } from '@wisp/ui';
import { useShell } from '../shell.ts';
import { dividerWords, switchOf } from './model.ts';

export default function SwitchDivider(props: ExtensionProps): JSX.Element {
  const shell = useShell();
  const row = useSignal(props.client.entities.row<unknown>('turn', (props.id ?? '') as never));
  const harnesses = useSignal(shell.harnesses());
  const nameOf = (id: string): string => harnesses().find((h) => h.id === id)?.name ?? id;
  const mark = (): ReturnType<typeof switchOf> => switchOf(row());
  return (
    <Show when={mark()}>
      {(m) => (
        <Divider
          part="turn.switch"
          label={dividerWords(m(), nameOf, (_a, model) => modelWords(model), shell.t)}
          logo={harnesses().find((h) => h.id === m().to.agent)?.logo}
        />
      )}
    </Show>
  );
}
