// The top-bar chip's gate (ADR-0105): Home's bar holds the chip, which shows only when it matters, and most of the time
// it doesn't. The gate reads the same cheap load row the chip does and loads the chip (its words, its view, its
// strings) only once there is something to show.
import type { ExtensionProps } from '@wisp/shell/contract';
import { Show, createMemo, lazy, type JSX } from 'solid-js';
import { chipInput, chipWhen } from './chip-when.ts';
import { useLoad } from './data.ts';

const Chip = lazy(() => import('./Chip.tsx'));

export default function ChipGate(props: ExtensionProps): JSX.Element {
  const { load, snap } = useLoad(props.client, props.client.home);
  const shows = createMemo(() => chipWhen(chipInput(load(), snap(), props.client.scope.timers.now())) !== null);
  return (
    <Show when={shows()}>
      <Chip {...props} />
    </Show>
  );
}
