// Machines from the avatar's menu (v1 #machinesBtn → openMachines): the Machines dialog as the shell's overlay, with
// the grid's part (reaching this computer from anywhere) as v1's had; its close is the overlay's.
import type { JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { MachinesDialog } from './panes/Manager.tsx';
import { MACHINES_VIEW } from './view-id.ts';

export default function MachinesView(props: { readonly client: PluginClient }): JSX.Element {
  return <MachinesDialog client={props.client} inOverlay withGrid onClose={() => props.client.navigate({ t: 'closeOverlay', id: MACHINES_VIEW as never })} />;
}
