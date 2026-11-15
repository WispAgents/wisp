// Follow-ups waiting for the run (v1 queued.js): inside the reply box over its text, before the ones set for a time,
// keyed by id so a reorder moves the rows and an edited row keeps its box. Each says when it goes (Next · after Claude
// Code finishes, Held after Stop, Queued #2); on hover it offers Send now, Combine with the next one and the pen, and ×
// is always there; a right-click (a tap on a touch screen) is its menu (Send now, Edit, Combine, Combine all, Move up /
// down, Cancel); two or more have a grip. Every change is a command with the queue's new order as its patch.
import { createMemo, createSignal, Show, type JSX } from 'solid-js';
import { Key } from '@zag-js/solid';
import { ContextMenu, QueuedAction, QueuedList, QueuedRow, TextArea, reordered, useQueueDrag, type MenuEntry } from '@wisp/ui';
import type { Env } from '../env.ts';
import { canCombineNext, combinableAll, editable, moved, quote, tagOf } from '../model/queue.ts';
import { plain } from '../text/mentions.ts';
import type { QueueItem, Row } from '../types.ts';
import '../i18n/lazy.ts';

const K = 'shell-composer.queue';

export interface QueueActions {
  now(id: string): void;
  edit(id: string, text: string): void;
  merge(ids: string[]): void;
  order(ids: string[]): void;
  cancel(id: string): void;
}

export interface QueueListProps {
  readonly env: Env;
  readonly queue: readonly Row<QueueItem>[];
  readonly held: boolean;
  readonly running: boolean;
  /** the task's harness, for the tags and titles */
  readonly name: string;
  readonly actions: QueueActions;
  /** a confirm that may be async (the shell's dialog); true goes ahead */
  readonly confirm: (text: string) => Promise<boolean>;
  /** Escape in an edit puts focus back in the reply box */
  readonly refocus: () => void;
}

function EditBox(p: { readonly env: Env; readonly text: string; readonly onDone: (text: string | undefined) => void }): JSX.Element {
  const [v, setV] = createSignal(p.text);
  const done = { sent: false };
  const finish = (text: string | undefined): void => {
    if (done.sent) return;
    done.sent = true;
    p.onDone(text);
  };
  return (
    <TextArea
      aria-label={p.env.t(`${K}.edit`)}
      value={v()}
      onInput={setV}
      autofocus
      minRows={1}
      maxRows={8}
      onBlur={() => finish(v())}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Escape') finish(undefined);
        else if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
          e.preventDefault();
          finish(v());
        }
      }}
    />
  );
}

export function QueueList(p: QueueListProps): JSX.Element {
  const t = p.env.t;
  const [editing, setEditing] = createSignal<string | undefined>(undefined);
  const ids = (): string[] => p.queue.map((q) => q.id);
  // the phone apps: long-press a follow-up, then drag it (design/mobile.md §A.4; twins: the grip, Move up / Move down)
  const drag = useQueueDrag({ item: "[data-part~='composer.queue.row']", ids, onReorder: (id, before) => p.actions.order(reordered(ids(), id, before)) });
  const cancel = async (q: Row<QueueItem>): Promise<void> => {
    if (await p.confirm(t(`${K}.cancel-confirm`, { text: quote(plain(q.text)) }))) p.actions.cancel(q.id);
  };
  // the row whose menu is open (a right-click, or a tap on a touch screen)
  const [target, setTarget] = createSignal<string | undefined>(undefined);
  const at = (): number => p.queue.findIndex((q) => q.id === target());
  const menu = (): MenuEntry[] => {
    const i = at();
    const q = p.queue[i];
    if (!q) return [];
    return [
      ...(q.id ? [{ value: 'now', label: t(`${K}.now`), icon: 'send' as const }] : []),
      ...(editable(q) ? [{ value: 'edit', label: t(`${K}.edit`), icon: 'pen' as const }] : []),
      ...(canCombineNext(p.queue, i) ? [{ value: 'merge', label: t(`${K}.merge`), icon: 'merge' as const }] : []),
      ...(editable(q) && combinableAll(p.queue).length ? [{ value: 'merge-all', label: t(`${K}.merge-all`, { count: combinableAll(p.queue).length }), icon: 'merge' as const }] : []),
      ...(i > 0 ? [{ value: 'up', label: t(`${K}.up`), icon: 'arrow' as const }] : []),
      ...(i < p.queue.length - 1 ? [{ value: 'down', label: t(`${K}.down`), icon: 'arrow' as const }] : []),
      { kind: 'separator' },
      { value: 'cancel', label: t(`${K}.cancel-menu`), icon: 'x', danger: true },
    ];
  };
  const pick = (v: string): void => {
    const i = at();
    const q = p.queue[i];
    if (!q) return;
    if (v === 'now') p.actions.now(q.id);
    else if (v === 'edit') setEditing(q.id);
    else if (v === 'merge') p.actions.merge([q.id, p.queue[i + 1]?.id ?? '']);
    else if (v === 'merge-all') p.actions.merge(combinableAll(p.queue));
    else if (v === 'up' || v === 'down') p.actions.order(moved(p.queue, i, v === 'up' ? i - 1 : i + 1));
    else if (v === 'cancel') void cancel(q);
  };
  return (
    <ContextMenu part="composer.queue.menu" items={menu()} onSelect={pick}>
      <QueuedList ref={drag} part="composer.queue" label={t(`${K}.label`)}>
        <Key each={p.queue} by={(q) => q.id}>
          {(q, i) => {
            const tag = (): string => {
              const tg = tagOf(i(), p.held, p.name);
              return t(tg.key, tg.vars);
            };
            // the box is made once per edit: a refresh of the row (its text, its place) keeps what you're typing
            const mine = createMemo(() => editing() === q().id);
            const edit = createMemo((): { box: JSX.Element; keys: string } | undefined =>
              mine()
                ? {
                    keys: t(`${K}.keys`),
                    box: (
                      <EditBox
                        env={p.env}
                        text={q().text}
                        onDone={(text) => {
                          setEditing(undefined);
                          if (text !== undefined && text.trim() && text.trim() !== q().text) p.actions.edit(q().id, text);
                          if (text === undefined) p.refocus();
                        }}
                      />
                    ),
                  }
                : undefined,
            );
            return (
              <QueuedRow
                part="composer.queue.row"
                tag={tag()}
                text={plain(q().text)}
                held={p.held}
                edit={edit()}
                onMenu={() => setTarget(q().id)}
                {...(editable(q()) && { onOpen: () => setEditing(q().id) })}
                {...(p.queue.length > 1 && { grip: { part: 'composer.queue.grip', label: t(`${K}.grip`), onMove: (by: number) => i() + by >= 0 && i() + by < p.queue.length && p.actions.order(moved(p.queue, i(), i() + by)) } })}
                cancel={{ label: t(`${K}.cancel-title`), onClick: () => void cancel(q()) }}
                actions={
                  <>
                    <Show when={q().id}>
                      <QueuedAction part="composer.queue.now" icon="send" text={t(`${K}.now`)} label={t(`${K}.now`)} title={p.running ? t(`${K}.now-running`, { name: p.name }) : t(`${K}.now`)} onClick={() => p.actions.now(q().id)} />
                    </Show>
                    <Show when={canCombineNext(p.queue, i())}>
                      <QueuedAction part="composer.queue.merge" icon="merge" label={t(`${K}.merge`)} title={t(`${K}.merge-title`)} onClick={() => p.actions.merge([q().id, ids()[i() + 1] ?? ''])} />
                    </Show>
                    <Show when={editable(q())}>
                      <QueuedAction part="composer.queue.pen" icon="pen" label={t(`${K}.edit`)} onClick={() => setEditing(q().id)} />
                    </Show>
                  </>
                }
              />
            );
          }}
        </Key>
      </QueuedList>
    </ContextMenu>
  );
}
