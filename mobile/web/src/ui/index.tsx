// The phone app's own UI (ADR-0065 step 8), registered with the shell like any plugin's: the Computers destination.
// Everything else the app shows is the shell and the plugins' UIs, drawn in the platform's look by the kit.
import type { PluginUiDef } from '@wisp/shell';
import { COMPUTERS_VIEW } from '@wisp/shell/pure';
import catalogue from './en.json' with { type: 'json' };
import { Computers, type ComputersDeps } from './Computers.tsx';

export function phoneUi(deps: ComputersDeps): PluginUiDef {
  return {
    id: 'wisp.phone',
    catalogue,
    views: [{ kind: COMPUTERS_VIEW, slot: 'main', parse: () => null, component: (p) => <Computers client={p.client} {...deps} /> }],
  };
}
