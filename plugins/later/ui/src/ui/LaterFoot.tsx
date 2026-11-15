// `wisp.later.foot`, inside the task's reply box over its text (`shell.task:composer-top`, v1 .c-queued): the follow-ups set for later in this task, soonest
// first, as one compact list (v1 #796 later.js followUpItems): a joined box with a short row each, its icon and time in
// a fixed column, what it says, the pen and ×; who set it and "in 18 min" are the tooltip. More than 3: the first 2 and
// "N more scheduled · next 14:52", which opens the rest (Show less closes it; opened per task until the page reloads).
// A click, right-click, Enter or Space on a row is its menu: Send now / Change time… / Cancel follow-up. A follow-up typed with a
// time in the reply box (F-auto-5) appears here at once: `later.add`'s patch. Before it goes it can be changed
// (F-auto-7, v1 0.49.0): the pen edits its text in place, Enter keeps it, Escape leaves it as it was.
import { Show, createMemo, createSignal, type JSX } from 'solid-js';
import { Key } from '@zag-js/solid';
import type { EntityKey } from '@wisp/client';
import type { ExtensionProps } from '@wisp/shell/contract';
import { ContextMenu, ScheduledButton, ScheduledEdit, ScheduledList, ScheduledMore, ScheduledRow, TextArea, useSignal, type IconName, type MenuEntry } from '@wisp/ui';
import { cancelFollowUp, editFollowUp, editedText } from '../commands.ts';
import { TimeMenu } from './TimeMenu.tsx';
import { aheadLabel, followUpTip, laterLine } from '../format.ts';
import { t } from '../i18n.ts';
import { KIND, type LaterView, type SessionRow } from '../types.ts';
import { footRows, sortLater } from '../rules.ts';
import '../i18n/lazy.ts';

/** The tasks whose whole list is open (v1 `opened`: per task, until the page reloads). */
const [opened, setOpened] = createSignal<ReadonlySet<string>>(new Set());

/** The text in place: Enter or leaving it keeps it, Escape leaves the follow-up as it was. */
function EditBox(p: { readonly text: string; readonly label: string; readonly onDone: (text: string | undefined) => void }): JSX.Element {
  const [v, setV] = createSignal(p.text);
  const done = { sent: false };
  const finish = (text: string | undefined): void => {
    if (done.sent) return;
    done.sent = true;
    p.onDone(text);
  };
  return (
    <TextArea
      aria-label={p.label}
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

/** The row's glyph (v1 #796: held, a reminder, else the clock) and what it says (a goal round in words). */
const iconOf = (l: LaterView): IconName => (l.held ? 'pause' : l.remind ? 'bell' : 'clock');
const textOf = (l: LaterView): string => (l.goal ? laterLine(l)[1] : l.text);

export default function LaterFoot(props: ExtensionProps): JSX.Element {
  const c = props.client;
  const key = (): EntityKey | undefined => (props.id ?? props.session?.()?.id) as EntityKey | undefined;
  type Row = SessionRow & { body?: { ext?: Record<string, { later?: LaterView[] } | undefined> } };
  const row = createMemo(() => {
    const k = key();
    return k ? useSignal(c.entities.row<Row>(KIND.session, k)) : () => undefined;
  });
  // the flat list (v1's row, and later.add's patch), else what wisp.later keeps in the task's ext slot
  const list = (): LaterView[] => sortLater(row()()?.later ?? row()()?.body?.ext?.['wisp.later']?.later ?? []);
  const open = (): boolean => opened().has(key() ?? '');
  const rows = createMemo(() => footRows(list(), open()));
  const toggle = (): void => {
    const k = key() ?? '';
    const next = new Set(opened());
    if (!next.delete(k)) next.add(k);
    setOpened(next);
  };
  const run = (p: Promise<unknown>): void => void p.catch(() => undefined); // the shell's onError shows the rollback toast
  const [editing, setEditing] = createSignal<string | undefined>(undefined);
  // the row whose menu is open, and the one whose Change time… menu is
  const [target, setTarget] = createSignal<LaterView | undefined>(undefined);
  const [timing, setTiming] = createSignal<string | undefined>(undefined);
  // a goal's round and a row not yet saved (its tmp id) aren't edited (v1 followUpItems)
  const editable = (l: LaterView): boolean => !l.goal && !l.id.startsWith('tmp');
  const editLabel = (l: LaterView): string => t(l.remind ? 'wisp.later.menu.editReminder' : 'wisp.later.menu.editFollowUp');
  const cancelLabel = (l: LaterView): string => t(l.remind ? 'wisp.later.menu.cancelReminder' : 'wisp.later.menu.cancelFollowUp');
  const send = (l: LaterView, args: { readonly now: true } | { readonly at: number }): void => run(c.run(editFollowUp, { sessionId: key() ?? '', item: l.id, ...args }));
  const cancel = (l: LaterView): void => run(c.run(cancelFollowUp, { sessionId: key() ?? '', item: l.id }));
  const menu = (): MenuEntry[] => {
    const l = target();
    if (!l) return [];
    const first: MenuEntry[] = [
      ...(l.goal ? [] : [{ value: 'now', label: t(l.remind ? 'wisp.later.menu.remindNow' : 'wisp.later.menu.sendNow'), icon: 'send' as const }]),
      ...(editable(l) ? [{ value: 'time', label: t('wisp.later.menu.changeTime'), icon: 'clock' as const }] : []),
    ];
    return [...first, ...(first.length ? [{ kind: 'separator' as const }] : []), { value: 'cancel', label: cancelLabel(l), icon: 'x', danger: true }];
  };
  const pick = (v: string): void => {
    const l = target();
    if (!l) return;
    if (v === 'now') send(l, { now: true });
    else if (v === 'time') setTiming(l.id);
    else if (v === 'cancel') cancel(l);
  };
  return (
    <Show when={list().length > 0}>
      <ContextMenu part="later.foot.menu" items={menu()} onSelect={pick}>
        <ScheduledList part="later.foot" label={t('wisp.later.foot.label')}>
          <Key each={rows().shown} by={(l) => l.id}>
            {(l) => (
              <Show
                when={editing() !== l().id}
                fallback={
                  <ScheduledEdit part="later.foot.edit">
                    <EditBox
                      text={l().text}
                      label={editLabel(l())}
                      onDone={(text) => {
                        setEditing(undefined);
                        const next = editedText(l().text, text);
                        if (next !== undefined) run(c.run(editFollowUp, { sessionId: key() ?? '', item: l().id, text: next }));
                      }}
                    />
                  </ScheduledEdit>
                }
              >
                <ScheduledRow
                  part="later.foot.row"
                  icon={iconOf(l())}
                  time={aheadLabel(l().at, Date.now())}
                  text={textOf(l())}
                  tip={followUpTip(l(), Date.now())}
                  held={!!l().held}
                  {...(l().files ? { files: { count: l().files ?? 0, tip: t('wisp.later.foot.files', { count: l().files ?? 0 }) } } : {})}
                  onMenu={() => setTarget(l())}
                  actions={
                    <>
                      <Show when={timing() === l().id}>
                        <TimeMenu
                          client={c}
                          part="later.foot.time"
                          open
                          onClose={() => setTiming(undefined)}
                          trigger={{ icon: 'clock', 'aria-label': t('wisp.later.menu.changeTime'), size: 'xs' }}
                          onPick={(at) => send(l(), { at })}
                        />
                      </Show>
                      <Show when={editable(l()) && timing() !== l().id}>
                        <ScheduledButton part="later.foot.pen" icon="pen" reveal label={editLabel(l())} onClick={() => setEditing(l().id)} />
                      </Show>
                      <ScheduledButton part="later.foot.cancel" icon="x" danger label={cancelLabel(l())} onClick={() => cancel(l())} />
                    </>
                  }
                />
              </Show>
            )}
          </Key>
          <Show when={rows().more}>
            {(m) => (
              <ScheduledMore
                part="later.foot.more"
                open={open()}
                label={open() ? t('wisp.later.foot.less') : t('wisp.later.foot.more', { n: m().rest, time: aheadLabel(m().next, Date.now()) })}
                onToggle={toggle}
              />
            )}
          </Show>
        </ScheduledList>
      </ContextMenu>
    </Show>
  );
}
