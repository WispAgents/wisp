// The open task's details as v1's .it-bar holds them (TaskView.tsx): the harness and model it runs on, in its project
// and folders, its status, on where it runs, its branch (a click copies the worktree's path), then its usage. Beside the
// turns on a computer, in a sheet from the menu on a phone (TaskTop.tsx).
import '../i18n/lazy.ts';
import { createMemo, type JSX } from 'solid-js';
import { worktreePath } from './change-where.ts';
import { useHere } from './use-here.ts';
import { TaskDetails } from '@wisp/ui';
import { modelWords } from './run-words.ts';
import { statusOf } from './rows-view.ts';
import { runFactory } from './run-factory.ts';
import { rowUsage, taskUsage, type TurnStatsLike } from './task-usage.ts';
import type { TaskPage } from './task-page.ts';
import type { SessionRow } from '../rows.ts';

/** The task's details, with what plugins pin beside the turns (`extra`, the computer's column only). */
export function TaskSide(props: { readonly p: TaskPage; readonly s: () => SessionRow; readonly extra?: JSX.Element }): JSX.Element {
  const { shell, t, turns, harnessView, placeChips } = props.p;
  const here = useHere();
  const { text, defaultName } = props.p.r;
  const s = props.s;
  // a factory run names its factory where another task names its harness (ADR-0114)
  const factory = (): string | undefined => runFactory(s().summary as { factory?: unknown });
  const factoryWords = (): string => t('shell.task.factory', { name: factory() ?? '' });
  // the task's own, for its details (not the reply box's pick)
  const own = (): { name: string; logo?: string } => (factory() ? { name: factoryWords() } : harnessView(s().agent));
  // on its harness's Default: the model that runs, by the same answer as the reply chip (QA r10 #2), else "Default model"
  const ownWords = (): string => factory() ? factoryWords() : t('shell.task.harness', { name: own().name, model: modelWords(String(s().summary.model || '')) || defaultName(s().agent, t('shell.task.defaultModel')) });
  const branch = (): string => String(s().summary['branch'] ?? '');
  // where it runs (row.ts runOf's words): a sandbox, a peer by name, else this computer
  const machine = (): { name: string; icon: 'box' | 'server' | 'laptop' } => {
    const m = s().summary['machine'] as { name?: string; kind?: string } | null | undefined;
    if (m?.kind === 'sandbox') return { name: m.name || t('shell.task.sandbox'), icon: 'box' };
    if (m) return { name: m.name || t('shell.task.remote'), icon: 'server' };
    return { name: here.name(), icon: here.icon() === 'server' ? 'server' : 'laptop' };
  };
  // the branch chip: a click copies the worktree's path (where the task's work is), the tooltip says so
  const branchChip = () => {
    const b = branch();
    const path = worktreePath(s());
    return {
      head: b.replace(/-[0-9a-f]{6}$/, ''),
      tail: b.match(/-[0-9a-f]{6}$/)?.[0] ?? '',
      title: path ? `${b}\n${path}` : b,
      ...(path && { copy: { text: path, done: t('shell.task.worktreeCopied') } }),
    };
  };
  const color = (): string => String(s().summary['color'] ?? 'slate');
  // its usage, its subagents' in, and how full its context is (v1 #785): the server sums them on the row
  const usage = createMemo(() =>
    taskUsage(
      rowUsage(s().summary as unknown as Readonly<Record<string, unknown>>),
      () => turns().map((x) => x.body.stats as TurnStatsLike | undefined),
      t,
      (id) => shell.navigate({ t: 'openTask', id: id as never }),
      () => shell.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane: 'usage' } } }),
    ),
  );
  return (
    <TaskDetails
      harness={ownWords()}
      {...(own().logo && { logo: own().logo })}
      project={String(s().summary['project'] ?? '')}
      color={color()}
      places={placeChips(s())}
      status={statusOf(s(), t)}
      machine={machine()}
      leads={{ project: t('shell.task.inProject'), machine: t('shell.task.onMachine') }}
      {...(branch() && { branch: branchChip() })}
      {...((u) => (u ? { usage: u } : {}))(usage())}
      actions={[]}
      {...(props.extra !== undefined && { extra: props.extra })}
    />
  );
}
