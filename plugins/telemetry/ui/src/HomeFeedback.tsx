// Home's last line (v1 split.js .sh-fb): "Hit a bug, or wish wisp did something? Tell us", opening Send feedback. Only
// while feedback can be sent: WISP_FEEDBACK=0 (the state row's `feedback`) takes it away, as v1's config.feedback did
// (B50). Until the row is there it shows, as feedback is on unless turned off.

import { Show, onCleanup, type JSX } from 'solid-js';
import { entityKey } from '@wisp/client';
import type { ExtensionProps } from '@wisp/shell/contract';
import { FeedbackLine, useSignal } from '@wisp/ui';
import { KIND, TOPIC, type StateRow } from './commands.ts';
import { t } from './i18n.ts';

export default function HomeFeedback(props: ExtensionProps): JSX.Element {
  const c = props.client;
  const scope = c.scope.child('home-feedback');
  onCleanup(() => scope.dispose());
  c.watch(c.home, TOPIC, scope, { passive: true });
  const row = useSignal(c.entities.row<StateRow>(KIND, entityKey(c.home, 'this')));
  const on = (): boolean => (row() as StateRow | undefined)?.feedback !== false;
  return (
    <Show when={on()}>
      <FeedbackLine label={t('telemetry.feedback.line')} onClick={() => c.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'shell.feedback' as never, params: {} } })} />
    </Show>
  );
}
