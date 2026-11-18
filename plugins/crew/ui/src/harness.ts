// The crew screens' harness label (`harness-label.ts`), following the harness list and the Auto switch.
import { entityKey, type PluginClient, type SettingRow } from '@wisp/client';
import { AUTO_SETTING, autoEnabled } from '@wisp/shell/pure';
import { useSignal } from '@wisp/ui';
import { harnessLabel, type Harness, type HarnessLabel } from './harness-label.ts';

export function useHarnessLabel(c: PluginClient, harnesses: () => readonly Harness[]): (id: string) => HarnessLabel {
  const row = useSignal(c.entities.row<SettingRow>('setting', entityKey(c.home, AUTO_SETTING)));
  return (id) => harnessLabel(id, harnesses(), autoEnabled(row()?.value));
}
