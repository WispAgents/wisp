// The Thread (variant A, picked 2026-11-18; v1 features/thread.js): on top, how worn the conversation is, one line
// ("Worn · context 85% full · 3 compactions · same error 4×", hidden when Fine) with Fresh thread when it's worn;
// under it the Notes section by section, each with who wrote it and when, editable in place; the empty sections fold
// into one Add row. Plain props, no client: Thread.tsx feeds it.
import { Show, createMemo, createSignal, type JSX } from 'solid-js';
import { Button, Dot, EmptyState, ForKeyed, Inline, Stack, Text, Wrap } from '@wisp/ui';
import type { Key, Vars } from './i18n.ts';
import { healthFacts, splitSections, toneOf, type SectionId, type SectionView } from './model.ts';
import { Compactions } from './Audit.tsx';
import { SectionItem } from './Section.tsx';
import type { Audit, Health, Revision } from './types.ts';
import './i18n/lazy.ts';

type T = (key: Key, vars?: Vars) => string;

export interface ThreadViewProps {
  readonly sections: readonly SectionView[];
  readonly revisions: (section: SectionId, current: number) => readonly Revision[];
  readonly health?: Health | undefined;
  /** what each compaction kept of the Notes, newest first (none while the setting is off) */
  readonly audits?: readonly Audit[];
  /** a subagent's: its lead's Notes, which it can't change */
  readonly readOnly: boolean;
  /** you changed them since the agent last read them */
  readonly unseen: boolean;
  /** Fresh thread shows: worn, the task's own conversation at rest, and its wisp offers threads.fresh */
  readonly canFresh: boolean;
  readonly now: number;
  readonly onSave: (section: SectionId, text: string) => void;
  readonly onFresh: () => void;
  readonly t: T;
}

export function HealthLine(props: { readonly health: Health; readonly canFresh: boolean; readonly onFresh: () => void; readonly t: T }): JSX.Element {
  const word = (): string => props.t(`threads.level.${props.health.level}`);
  const facts = (): string => healthFacts(props.health).map((f) => props.t(f.key, f.vars)).join(' · ');
  // mock A: the level and what made it so on one line, Fresh thread under it (so the facts keep the line's width)
  return (
    <Stack gap="3" part="threads.health" aria-label={props.t('threads.health.label')}>
      <Inline gap="3" align="center" part="threads.health.line">
        <Dot tone={toneOf(props.health.level)} size="md" part="threads.health.dot" />
        <Text weight="semibold" size="sm" nowrap part="threads.health.level">
          {word()}
        </Text>
        <Show when={facts()}>
          <Text size="sm" tone="faint">
            ·
          </Text>
          <Text size="sm" tone="muted" truncate numeric title={facts()} part="threads.health.facts">
            {facts()}
          </Text>
        </Show>
      </Inline>
      <Show when={props.canFresh}>
        <Inline gap="2" part="threads.health.actions">
          <Button part="threads.fresh" variant={props.health.level === 'worn' ? 'primary' : 'ghost'} size="sm" icon="replay" title={props.t('threads.fresh.tip')} onClick={() => props.onFresh()}>
            {props.t('threads.freshThread')}
          </Button>
        </Inline>
      </Show>
    </Stack>
  );
}

function AddRow(props: { readonly empty: readonly SectionView[]; readonly onAdd: (id: SectionId) => void; readonly t: T }): JSX.Element {
  return (
    <Wrap gap="2" part="threads.add">
      <ForKeyed each={props.empty}>
        {(s) => (
          <Button part="threads.add.item" variant="ghost" size="xs" icon="plus" title={props.t('threads.add', { section: props.t(`threads.section.${s().id}`) })} onClick={() => props.onAdd(s().id)}>
            {props.t(`threads.section.${s().id}`)}
          </Button>
        )}
      </ForKeyed>
    </Wrap>
  );
}

export function ThreadView(props: ThreadViewProps): JSX.Element {
  const [editing, setEditing] = createSignal<SectionId | undefined>();
  const split = createMemo(() => splitSections(props.sections));
  // what shows as a section: the written ones, and an empty one while it's open as a box (in its place in the order)
  const shown = createMemo(() => props.sections.filter((s) => s.text.trim() !== '' || s.id === editing()));
  const empty = createMemo(() => split().empty.filter((s) => s.id !== editing()));
  const save = (id: SectionId, text: string): void => {
    setEditing(undefined);
    props.onSave(id, text);
  };
  return (
    <Stack gap="5" part="threads.thread" aria-label={props.t('threads.label')}>
      <Show when={props.health && props.health.level !== 'ok' && props.health}>{(h) => <HealthLine health={h()} canFresh={props.canFresh} onFresh={props.onFresh} t={props.t} />}</Show>
      <Show when={props.readOnly}>
        <Text size="sm" tone="muted" part="threads.lead">
          {props.t('threads.lead')}
        </Text>
      </Show>
      <Show when={props.unseen && !props.readOnly}>
        <Text size="sm" tone="muted" part="threads.unseen">
          {props.t('threads.unseen')}
        </Text>
      </Show>
      <Show when={shown().length > 0} fallback={<EmptyState part="threads.empty" title={props.t('threads.empty')} compact icon="text" />}>
        <Stack gap="6" part="threads.sections">
          {/* keyed by section: a rewrite of one updates it in place, the others stay as they are */}
          <ForKeyed each={shown()}>
            {(s) => (
              <SectionItem
                section={s()}
                revisions={props.revisions(s().id, s().rev)}
                readOnly={props.readOnly}
                now={props.now}
                editing={editing() === s().id}
                onEdit={(on) => setEditing(on ? s().id : undefined)}
                onSave={(text) => save(s().id, text)}
                t={props.t}
              />
            )}
          </ForKeyed>
        </Stack>
      </Show>
      <Show when={!props.readOnly && empty().length > 0}>
        <AddRow empty={empty()} onAdd={setEditing} t={props.t} />
      </Show>
      <Compactions audits={props.audits ?? []} now={props.now} t={props.t} />
    </Stack>
  );
}
