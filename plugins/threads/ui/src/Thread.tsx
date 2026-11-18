// The Thread for one task (the side tab's body, and the sheet from the task's menu): its Notes from the
// `threads_notes` rows on its `session:<id>` (a subagent's: its lead's, through threads.notes, read-only), each
// section's versions from `threads_revisions`, the health from its `threads_health` row (worked out on the spot when
// it has none), and Fresh thread when the conversation is worn and this task's wisp offers `threads.fresh`.
import { createMemo, createResource, createSignal, onCleanup, type JSX } from 'solid-js';
import { entityKey, ownerOf, type EntityKey, type PluginClient } from '@wisp/client';
import type { Topic } from '@wisp/protocol';
import { useShell } from '@wisp/shell';
import { Box, useDerived, useSignal } from '@wisp/ui';
import { notesUpdate, readHealth, readNotes } from './commands.ts';
import { FreshDialog } from './Fresh.tsx';
import { t } from './i18n.ts';
import './i18n/lazy.ts';
import { TICK_MS, auditsOf, freshable, revisionsOf, sectionsOf, yoursUnseen, type SectionId, type TaskLike } from './model.ts';
import { KIND, PLUGIN, type AuditRow, type HealthRow, type MetaRow, type NoteRow, type RevisionRow } from './types.ts';
import { ThreadView } from './ThreadView.tsx';


export function Thread(props: { readonly client: PluginClient; readonly task: string }): JSX.Element {
  const shell = useShell();
  const key = props.task as EntityKey;
  const peer = ownerOf(key);
  const id = key.slice(key.indexOf(':') + 1);
  // its rows come with the task's own topic: kept live while this shows (the open task's view already has it: one
  // subscription between them)
  const scope = props.client.scope.child(`threads:${id}`);
  onCleanup(() => scope.dispose());
  shell.client.peers.watch(peer, `session:${id}` as Topic, scope);
  const [now, setNow] = createSignal(Date.now());
  scope.interval(TICK_MS, () => setNow(Date.now()));

  const row = useSignal(props.client.entities.row<TaskLike>(KIND.session, key));
  const notes = useSignal(props.client.entities.list<NoteRow>(KIND.notes));
  const revs = useSignal(props.client.entities.list<RevisionRow>(KIND.revisions));
  const meta = useSignal(props.client.entities.row<MetaRow>(KIND.meta, entityKey(peer, id)));
  const auditRows = useSignal(props.client.entities.list<AuditRow>(KIND.audits));
  const healthRow = useSignal(props.client.entities.row<HealthRow>(KIND.health, entityKey(peer, id)));
  const lead = (): boolean => !!row()?.parent_id;
  const [leads] = createResource(
    () => (lead() ? key : undefined),
    (k) => props.client.run(readNotes, { key: k }).catch(() => undefined),
  );
  const [worked] = createResource(
    () => (row() && !healthRow() ? key : undefined),
    (k) => props.client.run(readHealth, { key: k }).catch(() => undefined),
  );
  const sections = createMemo(() => (lead() ? sectionsOf((leads()?.sections ?? []).map((s) => ({ ...s, section: s.id }))) : sectionsOf(notes() as readonly NoteRow[], id)));
  const health = () => healthRow()?.body ?? worked();
  // features come from the peer (plan §7.3.2): Fresh thread waits for a wisp.threads that has threads.fresh
  const offersFresh = useDerived(() => shell.client.peers.hasFeature(peer, { plugin: PLUGIN, command: 'threads.fresh' }).ok);
  const canFresh = (): boolean => health()?.level === 'worn' && offersFresh() && freshable(row());
  const [freshOpen, setFreshOpen] = createSignal(false);
  const save = (section: SectionId, text: string): void => {
    void props.client.run(notesUpdate, { key, section, text }).catch(() => undefined); // a refusal: rolled back, toasted
  };
  return (
    <Box part="threads.pane" pad="6">
      <ThreadView
        sections={sections()}
        revisions={(s, current) => revisionsOf(revs() as readonly RevisionRow[], id, s, current)}
        health={health()}
        audits={auditsOf(auditRows() as readonly AuditRow[], id, now())}
        readOnly={lead()}
        unseen={yoursUnseen(meta() as MetaRow | undefined, sections())}
        canFresh={canFresh()}
        now={now()}
        onSave={save}
        onFresh={() => setFreshOpen(true)}
        t={t}
      />
      <FreshDialog client={props.client} task={key} open={freshOpen()} onOpenChange={setFreshOpen} t={t} />
    </Box>
  );
}
