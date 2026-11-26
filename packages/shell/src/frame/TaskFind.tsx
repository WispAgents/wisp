// Find in a task (mod+f, or Find in task in the task's menu on a phone; R4-3): the bar over the turns, its own chunk,
// loaded the first time it opens (ADR-0011). The loaded turns are searched here, in their step rows (task-find.ts);
// the turns the window sent as stubs are searched on the server (`runtime.searchTask`). Going to a hit is the task
// page's (`go`: its turn opened, its cluster and step opened, scrolled to); the bar marks the words (find-mark.ts).
import { createEffect, createMemo, createSignal, on, onCleanup, untrack, type JSX } from 'solid-js';
import { FindBar, markFind } from '@wisp/ui';
import type { ShellContext } from '../contract.ts';
import { searchTask } from './commands.ts';
import { findInTurns, landed, mergeHits, type FindHit, type FindTurn } from './task-find.ts';
import '../i18n/lazy.ts';

/** How long typing rests before the server is asked about the older turns. */
const ASK_MS = 200;

export interface TaskFindProps {
  readonly shell: ShellContext;
  readonly task: string;
  readonly turns: () => readonly FindTurn[];
  /** brings the hit into sight: its step's element once drawn, nothing while its turn is still on its way */
  readonly go: (hit: FindHit) => Promise<Element | undefined>;
  /** where the other hits are marked: the turns */
  readonly scope: () => Element | undefined;
  readonly onClose: () => void;
}

export default function TaskFind(props: TaskFindProps): JSX.Element {
  const t = props.shell.t;
  const [q, setQ] = createSignal('');
  const [at, setAt] = createSignal(-1);
  const [remote, setRemote] = createSignal<readonly FindHit[]>([]);
  const [busy, setBusy] = createSignal(false);
  // a stubbed turn's hit gone to: its turn is opening, and its own hit replaces it once it lands
  let waiting: FindHit | undefined;
  let unmark: (() => void) | undefined;
  const stubs = createMemo(() => props.turns().filter((x) => x.stub).map((x) => x.turnId), undefined, { equals: (a, b) => a.join() === b.join() });
  const local = createMemo(() => findInTurns(props.turns(), q()));
  const hits = createMemo(() => mergeHits(props.turns(), local(), remote()));

  // the older turns, asked once typing rests; an answer for words since changed is dropped
  createEffect(
    on([q, stubs], ([words, ids]) => {
      setRemote([]);
      if (!words.trim() || ids.length === 0) return setBusy(false);
      setBusy(true);
      const timer = setTimeout(() => {
        props.shell.client.commands.run(searchTask, { id: props.task, query: words, turnIds: ids }).then(
          (out) => {
            if (untrack(q) !== words) return;
            setRemote(out.hits.map((h) => ({ turnId: h.turnId, seq: h.seq, snippet: h.snippet })));
            setBusy(false);
          },
          () => untrack(q) === words && setBusy(false),
        );
      }, ASK_MS);
      onCleanup(() => clearTimeout(timer));
    }),
  );

  const mark = (el: Element | undefined): void => {
    unmark?.();
    const scope = props.scope();
    unmark = scope ? markFind(scope, el, q().trim()) : undefined;
  };
  const goTo = (i: number): void => {
    const h = hits()[i];
    setAt(i);
    if (!h) return mark(undefined);
    waiting = h.stepId === undefined ? h : undefined;
    void props.go(h).then((el) => {
      if (untrack(at) === i && untrack(hits)[i] === h) mark(el);
    });
  };
  const step = (by: 1 | -1): void => {
    const n = hits().length;
    if (n === 0) return;
    goTo(at() < 0 ? (by > 0 ? 0 : n - 1) : (at() + by + n) % n);
  };
  // new words: back to before the first hit, the page's marks follow the words
  createEffect(
    on(q, () => {
      waiting = undefined;
      setAt(-1);
      mark(undefined);
    }, { defer: true }),
  );
  // a stubbed turn gone to landed whole: its own hit takes the server's place
  createEffect(
    on(hits, (list) => {
      if (!waiting || list.some((h) => h === waiting)) return;
      const i = landed(list, waiting);
      if (i >= 0) goTo(i);
    }),
  );
  onCleanup(() => unmark?.());

  const count = (): string => {
    if (!q().trim()) return '';
    const n = hits().length;
    if (n === 0) return busy() ? t('shell.find.looking') : t('shell.find.none');
    return at() >= 0 ? t('shell.find.of', { n: at() + 1, m: n }) : t(n === 1 ? 'shell.find.count.one' : 'shell.find.count.other', { n });
  };
  return (
    <FindBar
      part="task.find"
      value={q()}
      onInput={setQ}
      count={count()}
      busy={busy()}
      onNext={() => step(1)}
      onPrev={() => step(-1)}
      onClose={props.onClose}
      labels={{ placeholder: t('shell.find.placeholder'), next: t('shell.find.next'), prev: t('shell.find.prev'), close: t('shell.find.close') }}
    />
  );
}
