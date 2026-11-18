// Under a compaction's turn (`shell.turn:foot`, the turn's key as its id): what the compaction kept of the Notes
// ("Kept 6 of 8 after the compaction") and what it lost (ADR-0096). Nothing when the turn has no audit.
import { Show, createSignal, onCleanup, type JSX } from 'solid-js';
import type { ExtensionProps } from '@wisp/shell/contract';
import { useSignal } from '@wisp/ui';
import { AuditLine } from './Audit.tsx';
import { t } from './i18n.ts';
import './i18n/lazy.ts';
import { TICK_MS, auditFor } from './model.ts';
import { KIND, type AuditRow } from './types.ts';

export default function AuditFoot(props: ExtensionProps): JSX.Element {
  const turn = (): string => {
    const id = props.id ?? '';
    return id.slice(id.indexOf(':') + 1);
  };
  const rows = useSignal(props.client.entities.list<AuditRow>(KIND.audits));
  const [now, setNow] = createSignal(Date.now());
  onCleanup(props.client.scope.interval(TICK_MS, () => setNow(Date.now())));
  const audit = () => auditFor(rows() as readonly AuditRow[], turn(), now());
  return <Show when={audit()}>{(a) => <AuditLine audit={a()} now={now()} t={t} />}</Show>;
}
