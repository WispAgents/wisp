// The open task as v1 shows it (web/js/features/split.js, feed.js openBody, transcript.js): its head,
// its turns (the ask, the answer's brief, the trail), the details column with the task's actions,
// the overlap note and the reply box. It renders at once from the list's row (design/feedback.md);
// the turns fill in as the task's own topic arrives. Every control answers at once (design/feedback.md):
// Mark done, a follow-up, Rerun and Fork show their result before the peer answers; Ship it and a
// restore show their control working; the panes and menus open in the same frame.
import { Show, onCleanup, type JSX } from 'solid-js';
import { TaskBody, TaskScreen } from '@wisp/ui';
import { Slot, useAnyShown } from '../slot.tsx';
import { renderTaskTurn, type TurnCtx } from './TaskTurn.tsx';
import type { TurnRow } from './task-rows.ts';
import { useTaskPage } from './task-page.ts';
import { TaskSide } from './TaskSide.tsx';
import { TaskTurns } from './TaskTurns.tsx';
import { TaskDock } from './TaskDock.tsx';
import { TaskTop } from './TaskTop.tsx';
import '../i18n/lazy.ts';
import '../i18n/home.ts';

/** The open task: its page's state (task-page.ts) drawn as its head, its body (details, turns, dock) and its questions. */
export default function TaskView(props: { readonly id: string; readonly onBack?: () => void }): JSX.Element {
  const p = useTaskPage(props);
  const { t, key, chat, built, phoneBar, setPaneWide, onBodyScroll, setBodyEl, setScrollCtrl, asks } = p;
  return (
    <Show when={p.row()}>
      {(s) => {
        // what takes the body's place (a terminal task's CLI): its extensions' `if` read the task's row
        const screen = useAnyShown('shell.task:screen', { id: key, session: () => s() as never });
        // one turn whole: the latest (the live one) after the ledger, or an earlier one opened in it (TaskTurn.tsx)
        const turnCtx: TurnCtx = { ...p, replyRun: p.r.replyRun, reply: p.r.reply, draft: p.r.draft, s };
        const renderTurn = (tr: () => TurnRow): JSX.Element => renderTaskTurn(turnCtx, tr);
        return (
          <>
            {/* a terminal task's CLI (shell-task's, v1 .tui-term) takes the body's place, edge to edge: `shell.task:screen` */}
            <Show
              when={!screen()}
              fallback={
                <TaskScreen>
                  <Slot input="shell.task:screen" layout="stack" id={key} session={() => s() as never} />
                </TaskScreen>
              }
            >
        <TaskBody
          chatWidth={chat.chatWidth()}
          chat={chat.chatMode()}
          toLatest={t('shell.task.toLatest')}
          scrollRef={setScrollCtrl}
          bodyRef={(el) => {
            setBodyEl(el);
            el.addEventListener('scroll', onBodyScroll, { passive: true });
            onCleanup(() => el.removeEventListener('scroll', onBodyScroll));
            // the pane the kit's `pane` container query reads: wide at 860 × 520 and up, as v1's split.js pane-wide
            // (looked up once the body is in the page: a ref runs before it's inserted)
            if (typeof ResizeObserver !== 'undefined') {
              const ro = new ResizeObserver(([e]) => {
                const b = e?.contentBoxSize[0];
                if (b) setPaneWide(b.inlineSize >= 860 && b.blockSize >= 520);
              });
              queueMicrotask(() => {
                const pane = el.closest('[data-kit="split-main"]');
                if (pane) ro.observe(pane);
              });
              onCleanup(() => ro.disconnect());
            }
          }}
                {...(!phoneBar() && {
                  details: (
                    <Show when={built()}>
                      {/* pinned beside the turns, in sight however far they scroll (wisp.ship's PR card) */}
                      <TaskSide p={p} s={s} extra={<Slot input="shell.task:details" layout="stack" id={key} session={() => s() as never} />} />
                    </Show>
                  ),
                })}
                turns={<TaskTurns p={p} s={s} renderTurn={renderTurn} />}
                dock={<TaskDock p={p} s={s} />}
              />
            </Show>
            <TaskTop p={p} s={s} screen={() => !!screen()} />
            {asks()}
          </>
        );
      }}
    </Show>
  );
}
