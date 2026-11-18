// The task list (v1's feed in the sidebar): the project chips, what plugins put on top, then every
// section in one VirtualList (5k tasks: only the rows on screen exist), with one shared row menu.
// Server data comes from the entity store through the model's memoised sections; what's open comes
// from navigation; the rest (whose menu is open, the dialogs) is local to this list.

import { createEffect, createMemo, createSignal, For, Match, on, onCleanup, Show, Switch, type JSX } from 'solid-js';
import { ContextMenu, EmptyState, ProjectHead, Skeleton, Stack, VirtualList, feedDoneColumn, useDerived, useSignal, type MenuEntry } from '@wisp/ui';
import { listFace } from '../model/list-face.ts';
import type { ProjectColor } from '@wisp/tokens';
import type { EntityKey } from '@wisp/client';
import type { ExtensionProps, ResolvedExtension } from '@wisp/shell/contract';
import { copyId, setUnread, toggleArchived, toggleDone, toggleForLater, togglePin } from '../actions.ts';
import { flatten, itemKey, sectionGroups, taskKeys, type Item } from '../model/flat.ts';
import { pickOf } from '../model/sections.ts';
import { createPull } from '../model/pull.ts';
import { rowMenu } from '../model/menu.ts';
import { anyShowsDone } from '../model/row.ts';
import { menuEntryOf } from '@wisp/shell';
import { flatSession } from '../model/wire.ts';
import { useShell } from '../shell.ts';
import { KIND, type Session, type SessionRow } from '../types.ts';
import { buildEnv } from './build-env.ts';
import { DeleteDialog } from './Dialogs.tsx';
import { Env, useEnv, type ListEnv } from './env.ts';
import { GroupRow, PlanRow, SectionHead } from './OtherRows.tsx';
import { revealIn } from './reveal.ts';
import { Slot, hasExtensions } from './Slot.tsx';
import { TaskRow } from './TaskRow.tsx';
import '../i18n/lazy.ts';

/** px of a row not measured yet: three lines (title, what it's doing, where it runs) */
export const ROW_ESTIMATE = 58;

export function TaskList(props: ExtensionProps): JSX.Element {
  const env = buildEnv(useShell(), props.client);
  // the list's card, lent while the list shows: the phone's Crew section nests a member's jobs with it (v1 placeCards)
  onCleanup(
    env.shell.taskCards?.provide((key) => (
      <Env.Provider value={env}>
        <TaskRow k={key as EntityKey} />
      </Env.Provider>
    )) ?? (() => {}),
  );
  return (
    <Env.Provider value={env}>
      <ListBody />
    </Env.Provider>
  );
}

function ListBody(): JSX.Element {
  const env = useEnv();
  const t = env.shell.t;
  const sections = useSignal(env.model.sections);
  const view = useSignal(env.model.view.value);
  // a crew member's page open: the list holds only their jobs (v1 crew.js inPick), with v1's empty line
  const nav = useSignal(env.shell.nav);
  const members = useSignal(env.client.entities.list<{ id: string; name: string }>('crew_member'));
  // the member's bare id: the page is keyed `<peer>:<id>`, while a member's row and a task's `crew.id` carry the bare
  // one (keyed, the list held none of their jobs and its empty line read "No tasks for yet.")
  const picked = createMemo(() => {
    const m = nav().main;
    if (m.kind !== 'crew') return undefined;
    const id = String(m.id);
    return id.slice(id.indexOf(':') + 1);
  });
  createEffect(() => env.model.setCrew(picked()));
  const emptyTitle = (): string => {
    const who = picked();
    if (who !== undefined) {
      const name = members().find((m) => m.id === who)?.name;
      return name ? t('shell-list.empty.crew', { name }) : t('shell-list.empty.all');
    }
    return t(view().filter !== 'all' ? 'shell-list.empty.project' : 'shell-list.empty.all');
  };
  const toggled = createMemo(() => new Set(view().toggled));
  // the crew's section (on the phone, where there's no rail): a plugin fills shell.list:crew. The phone apps draw it
  // under their Tasks top bar instead (the crew's stories, step 13b: the shell's PhoneListTop), never in the list
  const crew = hasExtensions('shell.list:crew');
  const crewInList = (): boolean => crew() && !env.shell.platform.phone;
  // a member's jobs for other tasks sit under the member, not in the sections (v1 crew.js): in the phone's Crew section,
  // or side by side in the face's card beside the rail
  const railCard = hasExtensions('shell.rail:card');
  createEffect(() => env.model.setNest(env.split() ? railCard() : crew()));
  // several projects picked: each section's rows grouped by project (none while a crew member is picked)
  const projects = useSignal(env.model.projects);
  const byProject = createMemo(() => {
    const known = new Set(projects().map((p) => p.id));
    return picked() === undefined ? pickOf(view().filter).filter((id) => known.has(id)) : [];
  });
  // the open task matters only beside the list (split: a folded section keeps its row); on a phone it changes nothing
  // here, so a tap doesn't re-flatten 4,000 tasks and re-plan the list behind the task page (F-phone-8)
  const open = createMemo(() => (env.split() ? env.open() : undefined));
  const items = createMemo(() => flatten(sections(), { toggled: toggled(), open: open(), split: env.split(), crew: !env.split() && crewInList(), byProject: byProject() }));
  // the order the list shows (its filters, folds and sort): a phone's task page steps through it (Next task, the head's
  // swipe) and the selection's Select all picks it (ADR-0065 step 13)
  createEffect(() => env.shell.listOrder?.publish(taskKeys(items())));
  // on touch, Mark done keeps a column of its own only while some row on the list shows it (v1 feed.js has-quick)
  const doneColumn = createMemo(() => !env.swipes && anyShowsDone(shownRows(items()), (k) => env.rowCtx(k as EntityKey)));
  const plugins = useSignal(env.shell.extensions('shell.row:menu'));
  const menu = useDerived<MenuEntry[]>(() => menuFor(env, plugins()));
  // the peer's list topic landed (this run, or saved whole at the last): an empty list is then really empty
  const loaded = useSignal(env.client.entities.loaded(env.client.home, 'list' as never));
  const refresh = pullRefresh(env, t);
  const box: { el?: HTMLElement } = {};
  // the keyboard's task (j / k, the palette) scrolls its row into view
  createEffect(on(env.selected, (k) => k !== undefined && box.el && revealIn(box.el, items(), k, ROW_ESTIMATE), { defer: true }));
  return (
    <Stack gap="2" grow part="list.sidebar">
      <Slot input="shell.list:top" layout="stack" />
      <Show
        when={items().length > 0}
        fallback={
          // still coming: the shape of rows, not "No tasks yet" (QA round 2 #12)
          <Show when={listFace(0, loaded()) === 'loading'} fallback={<EmptyState icon="spark" compact title={emptyTitle()} part="list.empty" />}>
            <Stack gap="7" part="list.loading">
              <For each={[0, 1, 2, 3, 4, 5]}>
                {() => (
                  <Stack gap="3">
                    <Skeleton width="2/3" />
                    <Skeleton width="1/2" />
                  </Stack>
                )}
              </For>
            </Stack>
          </Show>
        }
      >
        <ContextMenu
          items={menu()}
          onSelect={(v) => pick(env, v, plugins())}
          // the phone apps: Select first in a row's long-press menu enters the list's selection mode with that row
          {...(env.swipes && env.shell.listSelect && { onSelectMode: () => env.shell.listSelect?.start(env.menuFor()) })}
          part="list.menu"
        >
          <VirtualList
            ref={(el) => (box.el = el)}
            items={items()}
            key={itemKey}
            estimate={ROW_ESTIMATE}
            overscan={8}
            grow
            aria-label={t('shell-list.list.label')}
            part={'list.rows feed' as `${string}.${string}`}
            current={(it) => open() !== undefined && itemKey(it) === open()}
            vars={feedDoneColumn(doneColumn())}
            gapBefore={(i) =>
              isTaskItem(items()[i]) && isTaskItem(items()[i - 1])
                ? true
                : (items()[i]?.t === 'slot' && i > 0 && isTaskItem(items()[i - 1])) || (i > 0 && items()[i - 1]?.t === 'slot')
                  ? 'section'
                  : false
            }
            // each section a box from its heading to its last row (v1 #secNeeds, …): parity measures it, spells style it
            groups={sectionGroups(items())}
            refresh={refresh}
          >
            {(item) => <ItemView item={item()} />}
          </VirtualList>
        </ContextMenu>
      </Show>
      <DeleteDialog />
    </Stack>
  );
}

/** The task rows the list shows (a task, a group's open runs): the ones that may carry Mark done. */
function* shownRows(items: readonly Item[]): Generator<{ readonly key: string; readonly s: SessionRow }> {
  const runs = new Map<string, SessionRow>();
  for (const it of items) {
    if (it.t !== 'entry') continue;
    if (it.entry.kind === 'task') yield it.entry;
    else if (it.entry.kind === 'group') for (const r of it.entry.runs) runs.set(r.key, r.s);
  }
  for (const it of items) {
    if (it.t !== 'run') continue;
    const s = runs.get(it.key);
    if (s) yield { key: it.key, s };
  }
}

/** A task's row (its FeedItem): two in a row are 2px apart (v1 .f-list gap), a heading and a row are not. */
const isTaskItem = (it: Item | undefined): boolean => it?.t === 'entry' && it.entry.kind === 'task';

function ItemView(props: { readonly item: Item }): JSX.Element {
  const env = useEnv();
  const sections = useSignal(env.model.sections);
  const head = (): Extract<Item, { t: 'head' }> | undefined => (props.item.t === 'head' ? props.item : undefined);
  const entry = (): Extract<Item, { t: 'entry' }>['entry'] | undefined => (props.item.t === 'entry' ? props.item.entry : undefined);
  const nested = (): Extract<Item, { t: 'run' | 'sub' }> | undefined => (props.item.t === 'run' || props.item.t === 'sub' ? props.item : undefined);
  return (
    <Switch>
      <Match when={head()}>{(h) => <SectionHead id={h().id} count={h().count} open={h().open} entries={sections()[h().id]} />}</Match>
      <Match
        when={(() => {
          const e = entry();
          return e?.kind === 'task' ? e : undefined;
        })()}
      >
        {(e) => <TaskRow k={e().key} />}
      </Match>
      <Match
        when={(() => {
          const e = entry();
          return e?.kind === 'group' ? e : undefined;
        })()}
      >
        {(e) => <GroupRow entry={e()} />}
      </Match>
      <Match
        when={(() => {
          const e = entry();
          return e?.kind === 'plan' ? e : undefined;
        })()}
      >
        {(e) => <PlanRow plan={e().plan} />}
      </Match>
      <Match when={nested()}>{(n) => <TaskRow k={n().key} nested={n().t} />}</Match>
      <Match when={props.item.t === 'slot'}>
        <Slot input="shell.list:crew" layout="stack" />
      </Match>
      <Match when={props.item.t === 'proj' ? props.item : undefined}>{(j) => <ProjectGroupHead id={j().id} />}</Match>
    </Switch>
  );
}

/** A project's line over its rows (several picked): its dot and name, from the list's projects. */
function ProjectGroupHead(props: { readonly id: string }): JSX.Element {
  const env = useEnv();
  const projects = useSignal(env.model.projects);
  const p = createMemo(() => projects().find((x) => x.id === props.id));
  return <ProjectHead name={p()?.name ?? env.shell.t('shell-list.projects.other')} color={`pc-${p()?.color || 'slate'}` as ProjectColor} {...(p()?.path ? { title: p()?.path as string } : {})} />;
}

const sessionOf = (env: ListEnv, key: EntityKey): Session | undefined => env.client.entities.peek<Session>(KIND.session, key) as Session | undefined;

/** The open row's menu: the list's actions, and what plugins attached to shell.row:menu. It reads the row tracked
 *  (built in a useDerived): after Save for later or Archive it says Unsave / Unarchive at once (QA round 1 #7). */
export function menuFor(env: ListEnv, plugins: readonly ResolvedExtension[]): MenuEntry[] {
  const key = env.menuFor();
  const s = key === undefined ? undefined : (env.client.entities.row<Session>(KIND.session, key)() as Session | undefined);
  if (!key || !s) return [];
  // as the list reads it: summary and body flat (branch, toShip, pr…), its key in `id`
  const session = (): { id: string; agent: string } => ({ ...flatSession(s), id: key });
  const shown = plugins.filter((x) => x.def.bar && (x.def.if?.({ client: env.client, config: x.config, session }) ?? true));
  return rowMenu({ s, t: env.shell.t, plugins: shown.map((x) => menuEntryOf(x, { client: env.client, config: x.config, session }, env.shell.t, env.keys)), keys: env.keys });
}

/** A choice in the row menu. */
function pick(env: ListEnv, value: string, plugins: readonly ResolvedExtension[]): void {
  const key = env.menuFor();
  const s = key === undefined ? undefined : sessionOf(env, key);
  if (!key || !s) return;
  const a = env.actions;
  switch (value) {
    case 'open':
      return env.openTask(key);
    case 'rename':
      return env.askRename(key);
    case 'copy-id':
      return void copyId(a, key, (text) => env.shell.platform.copy(text));
    case 'later':
      return toggleForLater(a, key);
    case 'pin':
      return togglePin(a, key);
    case 'read':
      return setUnread(a, key, !s.unread);
    case 'done':
      return toggleDone(a, key);
    case 'archive':
      return toggleArchived(a, key);
    case 'delete':
      return env.askDelete(key);
    default: {
      const x = plugins.find((p) => `x:${p.id}` === value);
      x?.def.bar?.select?.({ client: env.client, config: x.config, session: () => ({ ...flatSession(s), id: key }) });
    }
  }
}

/** Pull to refresh (the phone apps): every computer reconnects now, skipping its backoff (ADR-0065 §4); the pull
 *  spins from the pull until the reconnect settles (model/pull.ts). Its twins: the me menu's Reconnect and the offline bar's Retry. */
function pullRefresh(env: ListEnv, t: (key: string) => string): { readonly onRefresh: () => void; readonly refreshing: boolean; readonly label: string } | undefined {
  if (!env.swipes) return undefined;
  const peers = env.client.peers;
  const connecting = useSignal(() => peers.list()().some((p) => peers.status(p)().kind === 'connecting'));
  const pull = createPull(connecting, () => peers.reconnect(), (ms, run) => env.client.scope.timeout(ms, run));
  return {
    onRefresh: pull.pull,
    get refreshing() {
      return pull.refreshing();
    },
    label: t('kit.gesture.reconnect'),
  };
}
