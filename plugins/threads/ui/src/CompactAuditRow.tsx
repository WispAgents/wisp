// Settings → General, in the tasks group (`shell.settings:tasks`, where v1 had it): Check compactions, the switch for
// `threads.compactAudit` (ADR-0096; declared by wisp.threads, off by default). Read from the home wisp's `setting`
// row, written with `settings.set`: it flips at once and a refusal rolls back with the client's toast; a plugin that
// locked it says so.
import type { JSX } from 'solid-js';
import { computed, entityKey, setSetting, type SettingRow } from '@wisp/client';
import type { ExtensionProps } from '@wisp/shell/contract';
import { SwitchSetting, type SettingSource } from '@wisp/ui';
import { t } from './i18n.ts';
import './i18n/lazy.ts';

export const COMPACT_AUDIT = 'threads.compactAudit';

export default function CompactAuditRow(props: ExtensionProps): JSX.Element {
  const row = props.client.entities.row<SettingRow>('setting', entityKey(props.client.home, COMPACT_AUDIT));
  const source: SettingSource<boolean> = {
    value: computed(() => row()?.value === true), // unset: off (its declared default)
    lockedBy: computed(() => row()?.locked_by ?? undefined),
    set: (on) => void props.client.run(setSetting, { peer: props.client.home, key: COMPACT_AUDIT, value: on }).catch(() => undefined),
  };
  return (
    <SwitchSetting
      part="threads.settings.compactAudit"
      title={t('threads.settings.compactAudit')}
      description={t('threads.settings.compactAudit.hint')}
      source={source}
      locked={(by) => t('threads.settings.locked', { by })}
    />
  );
}
