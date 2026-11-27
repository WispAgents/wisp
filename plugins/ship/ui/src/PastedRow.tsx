// Settings → General, in the tasks group (`shell.settings:tasks`): Follow PRs I link, the switch for
// `ship.attachPasted` (ADR-0145; declared by wisp.ship, on by default). A GitHub PR link in a prompt or follow-up
// attaches that PR to the task. Read from the home wisp's `setting` row, written with `settings.set`: it flips at
// once and a refusal rolls back with the client's toast; a plugin that locked it says so.
import type { JSX } from 'solid-js';
import { computed, entityKey, setSetting, type SettingRow } from '@wisp/client';
import type { ExtensionProps } from '@wisp/shell/contract';
import { SwitchSetting, type SettingSource } from '@wisp/ui';
import { t } from './i18n.ts';
import './i18n/lazy.ts';

export const ATTACH_PASTED = 'ship.attachPasted';

export default function PastedRow(props: ExtensionProps): JSX.Element {
  const row = props.client.entities.row<SettingRow>('setting', entityKey(props.client.home, ATTACH_PASTED));
  const source: SettingSource<boolean> = {
    value: computed(() => row()?.value !== false), // unset: on (its declared default)
    lockedBy: computed(() => row()?.locked_by ?? undefined),
    set: (on) => void props.client.run(setSetting, { peer: props.client.home, key: ATTACH_PASTED, value: on }).catch(() => undefined),
  };
  return (
    <SwitchSetting
      part="ship.settings.attachPasted"
      title={t('wisp.ship.settings.attachPasted')}
      description={t('wisp.ship.settings.attachPasted.hint')}
      source={source}
      locked={(by) => t('wisp.ship.settings.locked', { by })}
    />
  );
}
