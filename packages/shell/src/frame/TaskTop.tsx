// The open task's head (TaskView.tsx): in the browser and on a computer, v1's .split-head with its title, its slot
// and its one bar (TaskHead); on a phone's page, the platform's app bar with the task's menu, its status chips, the
// details sheet and the swipe to the next or previous task (ADR-0065 step 13).
import { Show, lazy, on, type JSX } from 'solid-js';
import { TaskHead } from '@wisp/ui';
import { prBadge } from './side-newest.ts';
import { TaskHeadSlot } from './slots.tsx';
import { phoneTaskMenu } from './phone-menu.ts';
import { rename } from './commands.ts';
import type { TaskPage } from './task-page.ts';
import type { SessionRow } from '../rows.ts';
import { TaskSide } from './TaskSide.tsx';

// the phone apps' task head (ADR-0065 step 13): the platform's app bar with the task's menu, in the phone heads' chunk
// (the list's head loaded it already)
const PhoneTaskBar = lazy(() => import('./PhoneHead.tsx').then((m) => ({ default: m.PhoneTaskBar })));

/** The head; `screen` says a terminal task's CLI has the body (the head then spans the page). */
export function TaskTop(props: { readonly p: TaskPage; readonly s: () => SessionRow; readonly screen: () => boolean }): JSX.Element {
  const {
    shell, t, key, row, run, chat, detailsOpen, setDetailsOpen, phoneBar, leave, nav, side, sideNewest, steps,
    stepTo, hasPr, diff, more, onMore, headBar,
  } = props.p;
  const { menu, when } = props.p.r;
  const s = props.s;
  const color = (): string => String(s().summary['color'] ?? 'slate');
  return (
    <Show
      when={phoneBar() && leave}
      fallback={
    <>
    <TaskHead
      actions={<TaskHeadSlot id={key} session={() => s() as never} />}
      title={s().title}
      color={color()}
      chatWidth={props.screen() ? 'full' : chat.chatWidth()}
      renameLabel={t('shell.task.rename')}
      panelLabel={t('shell.task.panel')}
      closeLabel={t('shell.task.close')}
      {...(leave && { onBack: leave, backLabel: t('shell.home.back') })}
      onClose={() => shell.navigate({ t: 'home' })}
      onRename={(title) => void run(rename, { id: key, title }).catch(() => undefined)}
      onPanel={sideNewest}
      panelOn={nav().side.kind !== 'none'}
      bar={headBar(s())}
    />
    </>
      }
    >
      {(back) => (
        <PhoneTaskBar
          title={s().title}
          onBack={back()}
          head={() => <TaskHeadSlot id={key} session={() => s() as never} />}
          // UX 2026-11-18 phone #6: the same entries in three groups, the rest in This task ›
          menu={() => phoneTaskMenu(more(), t)}
          onSelect={onMore}
          // UX 2026-11-18 phone #3: what the task holds, each a tap to its pane (a sheet over the task)
          status={() => [
            ...(diff().files > 0
              ? [{ id: 'changes', icon: 'file' as const, label: t('shell.phone.files', { count: diff().files, n: diff().files, plus: `+${diff().plus}`, minus: `−${diff().minus}` }), on: nav().side.kind === 'changes', onSelect: () => side('changes') }]
              : []),
            ...(hasPr() ? [{ id: 'pr', icon: 'branch' as const, label: ((b) => (b ? t('shell.phone.prNumber', { pr: b }) : t('shell.phone.pr')))(prBadge(row() as never)), on: nav().side.kind === 'pr', onSelect: () => shell.navigate({ t: 'toggleSide', side: { kind: 'pr', id: key as never } }) }] : []),
          ]}
          details={() => <TaskSide p={props.p} s={s} />}
          detailsOpen={detailsOpen()}
          onDetails={setDetailsOpen}
          swipe={{
            get onNext() {
              const to = steps().next;
              return to === undefined ? undefined : () => stepTo(to);
            },
            get onPrevious() {
              const to = steps().previous;
              return to === undefined ? undefined : () => stepTo(to);
            },
          }}
        />
      )}
    </Show>
  );
}
