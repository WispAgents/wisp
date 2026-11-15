// The Computers destination (ADR-0065 step 8, design/mobile.md §A.2): the saved computers, each with how it's reached
// and its state as a word, the launched one marked; a tap launches it (the shell runs on it and shows its tasks, in
// the same frame: launch() only switches the home and navigates). Kit components only, no CSS: each platform's
// look comes from the kit. Adding a computer (QR, SSH, wisp Cloud sign-in) is steps 13–14.
import { createSignal, For, onCleanup, Show, type JSX } from 'solid-js';
import type { PeerId, PluginClient } from '@wisp/client';
import { Button, EmptyState, ListRow, Pill, Scroll, Section, TopAppBar, useSignal } from '@wisp/ui';
import type { Tone } from '@wisp/tokens';
import type { Computer, NativeBridge } from '../bridge.ts';
import { computerRows, stateOf, type ComputerRow, type ComputerState } from '../computers.ts';
import type { Launcher } from '../launch.ts';
import { peerMap } from '../peers.ts';

const TONE: Record<ComputerState, Tone> = { online: 'good', connecting: 'info', offline: 'neutral', asleep: 'neutral', update: 'warn' };

export interface ComputersDeps {
  readonly bridge: Pick<NativeBridge, 'list' | 'onComputers'>;
  readonly launcher: Launcher;
}

function Row(props: { readonly client: PluginClient; readonly row: ComputerRow; readonly launcher: Launcher; readonly t: (k: string) => string }): JSX.Element {
  const status = useSignal(props.client.peers.status(props.row.peer));
  // the state follows the peer live: a row computed once would keep *Connecting*
  const state = (): ComputerState => stateOf(status());
  const open = (): void => void props.launcher.launch(props.row);
  return (
    <ListRow
      part="phone.computer"
      title={props.row.name}
      sub={props.t(`phone.via.${props.row.via}`)}
      current={props.row.current}
      strong={props.row.current}
      pills={
        <Pill tone={TONE[state()]} dot live={state() === 'connecting'} variant="plain" size="sm">
          {props.t(`phone.state.${state()}`)}
        </Pill>
      }
      actions={
        <Button variant={props.row.current ? 'secondary' : 'primary'} size="sm" part="phone.computer.open" onClick={open}>
          {props.t(props.row.current ? 'phone.tasks' : 'phone.open')}
        </Button>
      }
      onOpen={open}
    />
  );
}

export function Computers(props: { readonly client: PluginClient } & ComputersDeps): JSX.Element {
  const t = (k: string): string => props.client.t(k);
  const [list, setList] = createSignal<readonly Computer[]>([]);
  const load = (): void => void props.bridge.list().then(setList, () => undefined);
  load();
  onCleanup(props.bridge.onComputers(load));
  const home: PeerId = props.client.home; // the shell is run again on a switch, so this is read fresh
  const rows = (): ComputerRow[] => computerRows(list(), peerMap(list()), () => undefined, home);
  return (
    <>
      <TopAppBar kind="root" title={t('phone.computers.title')} />
      <Scroll grow part="phone.computers" aria-label={t('phone.computers.title')}>
        <Show when={rows().length > 0} fallback={<EmptyState icon="laptop" title={t('phone.computers.empty')}>{t('phone.computers.emptyHow')}</EmptyState>}>
          <Section title={t('phone.computers.saved')} description={t('phone.computers.help')}>
            <For each={rows()}>{(row) => <Row client={props.client} row={row} launcher={props.launcher} t={t} />}</For>
          </Section>
        </Show>
      </Scroll>
    </>
  );
}

export default Computers;
