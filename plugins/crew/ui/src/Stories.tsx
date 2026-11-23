// The crew on phones as stories (ADR-0065 step 13b, docs/design/crew-stories.html C, the owner's pick): in the phone
// apps the crew's section of the Tasks list (`shell.list:crew`) is a row of rings under the top bar, and a ring opens
// that member's story full screen (StoryViewer.tsx, lazy-loaded on first tap so the viewer and its commands stay off
// the phone first screen: ADR-0105).
import { createMemo, createSignal, lazy, Show, type JSX } from 'solid-js';
import type { ExtensionProps } from '@wisp/shell/contract';
import { CrewStories, useSignal, type StoryBubble } from '@wisp/ui';
import { crewData, emptyState, type CrewMember } from './data.ts';
import { t } from './i18n.ts';
import type { State } from './model.ts';
import { ringLabel, ringOf, storyOrder } from './stories.ts';
import './i18n/lazy.ts';

const StoryViewer = lazy(() => import('./StoryViewer.tsx'));

type Say = (k: string, v?: Readonly<Record<string, string | number>>) => string;
const say = t as Say;

export default function CrewStoriesView(props: ExtensionProps): JSX.Element {
  const c = props.client;
  const data = crewData(c);
  const ranked = useSignal(data.ranked);
  const states = useSignal(data.states);
  const stateOf = (m: CrewMember): State => states().get(m.id) ?? emptyState(!!m.paused);
  const order = createMemo(() => storyOrder(ranked(), states()));
  // one object per member until what it shows changes: the row keeps its rings (their turn, focus) as jobs stream
  let had = new Map<string, StoryBubble>();
  const bubbles = createMemo((): StoryBubble[] => {
    const next = new Map<string, StoryBubble>();
    // only who is working, asking or has a report for you: an idle member is on the Crew tab, not over your tasks
    // (UX 2026-11-18 phone #12; Org and Add are the Crew tab's too)
    const out = order().flatMap((m) => {
      const ring = ringOf(stateOf(m));
      if (ring === 'idle') return [];
      const b: StoryBubble = { id: m.id, name: m.name, avatar: m.avatar, color: m.color, ring, label: ringLabel(m.name, ring, say as never), dim: !!m.paused };
      const old = had.get(m.id);
      const same = old && old.name === b.name && old.avatar === b.avatar && old.color === b.color && old.ring === b.ring && old.label === b.label && old.dim === b.dim;
      const keep = same ? old : b;
      next.set(m.id, keep);
      return [keep];
    });
    had = next;
    return out;
  });
  // the story keeps the order it opened in, so a member never moves under your finger as rings change
  const [held, setHeld] = createSignal<readonly string[]>([]);
  const [open, setOpen] = createSignal<string | null>(null);
  const show = (id: string): void => {
    setHeld(bubbles().map((b) => b.id));
    setOpen(id);
  };
  return (
    <>
      <Show when={bubbles().length > 0}>
        <CrewStories label={t('wisp.crew.list.name')} members={bubbles()} onOpen={show} />
      </Show>
      <Show when={open()}>
        <StoryViewer client={c} data={data} order={order} states={states} held={held} open={open} setOpen={setOpen} />
      </Show>
    </>
  );
}
