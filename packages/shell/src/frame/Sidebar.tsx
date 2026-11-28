// The sidebar as v1 shows it (web/js/features/split.js, feed.js): its head (the brand, Search, the
// avatar, the sidebar button and the project chips), New task and My Stuff, then the list in its
// sections (Needs you, Working, Done), each task a row (the kit's TaskCard). The crew rail beside it.
// Opening a row navigates at once (design/feedback.md); Mark done is optimistic through the command.

import { For, Show, createMemo, createSignal, onCleanup, untrack, type JSX } from 'solid-js';
import { computed, ownerOf, type EntityKey, type Json } from '@wisp/client';
import { forLater, isSaved, markRead, setState } from './commands.ts';
import { listShown } from './pane-widths.ts';
import { openSwitcher } from './project-switch.ts';
import { computersNow, openComputers } from './runon-switch.ts';
import { ALL_COMPUTERS, hasComputerFilter, knownComputer } from './computer-filter.ts';
import { parsePick, pickLabel, pickOnly, pickRemove, pickValue, pickedProjects, pickHas } from './project-pick.ts';
import { displayCombo } from '../keys.ts';
import { CrewRail, EmptyState, Feed, FeedSection, PickStrip, SidebarHead, Skeleton, SplitTop, Stack, TaskCard, useSignal, type MenuEntry, type ProjectChip, type RailMember, type SidebarHeadProps } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { routeOf, routeTip } from './auto.ts';
import { useNow } from '../clock.ts';
import { SESSION, type SessionRow } from '../rows.ts';
import { rowSlots, useHeadSlots } from './slots.tsx';
import { harnessOf } from './harness.ts';
import { SECTIONS, crewOf, flatRow, goalOf, longTime, needsYou, overlapOf, projectOfRow, sectionOf, shortAgo, statusOf, sessionKey, type SectionId } from './rows-view.ts';
import { useProjects } from './projects.ts';
import { defaultModelName, runModelWord } from './default-models.ts';
import { meOf, type PeerDoc, type StatusDoc } from './me.ts';
import { Slot, useExtensions, usePluginClient } from '../slot.tsx';
import { useHere } from './use-here.ts';

interface CrewRow {
  readonly id: string;
  readonly name: string;
  readonly avatar?: string;
  readonly color?: string;
  /** when they joined: the rail shows them in joining order (v1's crew list), whatever order rows arrive in (B28) */
  readonly createdAt?: number;
}

function useSessions(): () => readonly SessionRow[] {
  const shell = useShell();
  const all = useSignal(shell.client.entities.list<SessionRow>(SESSION));
  return createMemo(() => all().filter((s) => !s.archived));
}

export function Rail(): JSX.Element {
  const shell = useShell();
  const crew = useSignal(shell.client.entities.list<CrewRow>('crew_member'));
  const sessions = useSessions();
  // who has a job running: their face gets a ring (ADR-0142)
  const busy = createMemo(() => new Set(sessions().flatMap((s) => (s.state === 'running' ? [crewOf(s)] : []))));
  // the crew host keeps a member in its row's body
  const members = createMemo<RailMember[]>(() =>
    crew()
      .map((r) => flatRow<CrewRow>(r))
      .toSorted((a, b) => (a.createdAt || 0) - (b.createdAt || 0) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
      .map((c) => ({ id: c.id, name: c.name, avatar: c.avatar ?? '', color: c.color ?? 'violet', live: busy().has(c.id) })),
  );
  const nav = useSignal(shell.nav);
  const cardExt = useExtensions('shell.rail:card');
  return (
    <CrewRail
      label={shell.t('shell.rail.label')}
      allLabel={shell.t('shell.rail.all')}
      orgLabel={shell.t('shell.rail.org')}
      addLabel={shell.t('shell.rail.add')}
      allOn={nav().main.kind !== 'crew'}
      orgOn={nav().main.kind === 'view' && String((nav().main as { id?: unknown }).id) === 'crew.org'}
      pickedId={(() => {
        const m = nav().main;
        return m.kind === 'crew' ? String(m.id) : undefined;
      })()}
      members={members()}
      onAll={() => shell.navigate({ t: 'home' })}
      onMember={(id) => shell.navigate({ t: 'openCrew', id: id as never })}
      onOrg={() => shell.navigate({ t: 'openView', id: 'crew.org' as never, params: null as Json })}
      // wisp.crew's editor (an overlay view; without that plugin, navigation drops it)
      onAdd={() => shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'crew.editor' as never, params: {} } })}
      // a face's card (v1 .cl-card), filled by whoever answers shell.rail:card (wisp.crew); none: no card
      {...(cardExt().length > 0 && {
        card: (id: string) => <Slot input="shell.rail:card" id={id} layout="contents" />,
        cardLabel: (id: string) => shell.t('shell.rail.cardLabel', { name: members().find((m) => m.id === id)?.name ?? '' }),
      })}
    />
  );
}

/** The head's project chips (All, then each project in your order) with their need-you counts, and the pick: the
 *  browser's head and the phone apps' chip row under the Tasks bar share them. */
export function useHeadChips(filter: () => string) {
  const shell = useShell();
  const sessions = useSessions();
  const pc = usePluginClient('shell');
  const { list: projects, reorder } = useProjects(shell, pc.home);
  const needs = (pred: (s: SessionRow) => boolean): number => sessions().filter((s) => needsYou(s) && pred(s)).length;
  const needLabel = (n: number): string => shell.t(n === 1 ? 'shell.side.need.one' : 'shell.side.need.other', { count: n });
  const chips = createMemo<ProjectChip[]>(() => {
    const all = needs(() => true);
    return [
      { id: 'all', name: shell.t('shell.side.allChip'), title: shell.t('shell.side.all'), need: all, needLabel: needLabel(all) },
      ...projects().map((p) => {
        const n = needs((s) => projectOfRow(s, projects())?.id === p.id);
        return { id: p.id, name: p.name, title: p.path, color: p.color || 'slate', need: n, needLabel: needLabel(n) };
      }),
    ];
  });
  // several projects picked: the button names them, their need-you counts summed, the tip listing each
  const pickIds = createMemo(() => knownPick(filter(), projects()));
  const pick = createMemo(() => {
    const ids = pickIds();
    if (ids.length < 2) return undefined;
    const picked = pickedProjects(ids, projects());
    const need = chips()
      .filter((c) => ids.includes(c.id))
      .reduce((n, c) => n + c.need, 0);
    return { colors: picked.map((p) => p.color || 'slate'), label: pickLabel(picked, shell.t), tip: picked.map((p) => p.name).join('\n'), need, needLabel: needLabel(need) };
  });
  const on = (): string => pickIds()[0] ?? 'all';
  return { chips, pick, pickIds, on, reorder, projects };
}

/** You on the grid and the me menu (v1 #meMenu): the browser's head and the phone apps' Tasks bar share it. */
export function useAccount() {
  const shell = useShell();
  const pc = usePluginClient('shell');
  // you on the grid (v1 split.js meInfo): wisp.machines' rows, on its topic while the head shows
  const meScope = pc.scope.child('sidebar-me');
  onCleanup(() => meScope.dispose());
  pc.watch(pc.home, 'machines' as never, meScope);
  const peers = useSignal(shell.client.entities.list<PeerDoc>('machines_peer'));
  const statuses = useSignal(shell.client.entities.list<StatusDoc>('machines_status'));
  const me = createMemo(() => meOf(peers(), statuses()));
  const nav = useSignal(shell.nav);
  // the theme as it is now, tracked (audit A F6: read bare, the menu kept the label of the theme it opened with)
  const theme = useSignal(shell.theme.resolved);
  // plugins' links (docs/ux/2026-11-18-grid.md #9: a cloud wisp's way back to its grid), those offered now
  const links = useSignal(
    computed(() =>
      (shell.accountLinks?.()() ?? []).flatMap((l) => {
        const href = l.href(l.client);
        return href ? [{ id: l.id, href, label: l.label(), icon: l.icon, ...(l.hint && { hint: l.hint() }) }] : [];
      }),
    ),
  );
  const sheet = (pane: string | undefined): void => shell.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: pane === undefined ? {} : { pane } } });
  // v1 #meMenu: you first, then Machines, Automations and Settings (the theme after, v2's). On a grid box, the grid's
  // link stands where "sign in to a grid" would: that wisp is on one already.
  const meRow = (): MenuEntry[] =>
    me().state === 'out' && links().length > 0
      ? []
      : [{ value: 'me', label: me().who || shell.t('shell.me.grid'), hint: me().state === 'out' ? shell.t('shell.me.out') : me().sub, icon: 'user', part: 'topbar.me' }];
  const items = (): MenuEntry[] => [
    ...meRow(),
    ...links().map((l): MenuEntry => ({ value: `link:${l.id}`, label: l.label, icon: l.icon, ...(l.hint !== undefined && { hint: l.hint }) })),
    { kind: 'separator' },
    { value: 'machines', label: shell.t('shell.account.machines'), icon: 'server' },
    { value: 'automations', label: shell.t('shell.account.automations'), icon: 'bolt' },
    // quick access to Usage (owner, 2026-11-27): Settings opened on its Usage pane
    { value: 'usage', label: shell.t('shell.account.usage'), icon: 'stats' },
    { value: 'settings', label: shell.t('shell.account.settings'), icon: 'settings', part: 'topbar.settings' },
    { kind: 'separator' },
    { value: 'theme', label: shell.t(theme() === 'dark' ? 'shell.cmd.theme.light' : 'shell.cmd.theme.dark'), icon: theme() === 'dark' ? 'sun' : 'moon' },
  ];
  // v1 meBtn's title: "<you> · Machines, Automations and Settings"
  const label = (): string => (me().who ? shell.t('shell.me.title', { who: me().who }) : shell.t('shell.side.account'));
  const onSelect = (v: string): void => {
    const href = me().href;
    // signed in: the grid's account page; signed out (or never): Settings → Cloud, to sign in
    if (v === 'me') return href && me().state !== 'out' ? shell.platform.openExternal(href) : sheet('cloud');
    const link = v.startsWith('link:') ? links().find((l) => `link:${l.id}` === v) : undefined;
    if (link) return shell.platform.openExternal(link.href);
    if (v === 'theme') void shell.theme.set(theme() === 'dark' ? 'light' : 'dark');
    // v1 #machinesBtn → openMachines: the Machines dialog (wisp.machines' overlay view); This computer is in Settings
    else if (v === 'machines') shell.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'machines.dialog' as never, params: {} as Json } });
    // Settings on a phone opens on its list, as v1's did; a wider window shows General beside the list
    else sheet(v === 'automations' || v === 'usage' ? v : nav().layout === 'stack' ? undefined : 'general');
  };
  return { me, items, label, onSelect };
}

/** The project the list shows (All is 'all'): the chips set it, the list follows. */
export function Head(props: { readonly filter: () => string; readonly setFilter: (id: string) => void }): JSX.Element {
  const shell = useShell();
  const here = useHere();
  const { chips, pick, on, reorder } = useHeadChips(props.filter);
  const account = useAccount();
  const me = account.me;
  const slots = useHeadSlots();
  const nav = useSignal(shell.nav);
  const homeOn = (): boolean => nav().main.kind === 'home';
  const listSet = useSignal(() => shell.sidebar.open() !== false);
  // shown, as the frame shows it: the side panel takes the list's place in a window under 1180 (pane-widths listShown)
  const listOpen = (): boolean => listShown(listSet(), nav().layout === 'split' && nav().main.kind !== 'home' && nav().side.kind !== 'none', window.innerWidth);
  // the computer filter beside the project's (owner, 2026-11-27): All computers or one, from two computers on
  const computers = useSignal(() => computersNow(shell));
  const computerOn = useSignal(() => knownComputer(shell.computerFilter?.value(), computersNow(shell)));
  const computer = (): SidebarHeadProps['computer'] => {
    if (!hasComputerFilter(computers())) return undefined;
    const on = computerOn();
    const label = on === ALL_COMPUTERS ? shell.t('shell.phone.allComputers') : on === shell.client.home() ? here.name() : shell.client.peers.name(on as never) || on;
    return { label, colors: [], icon: on === ALL_COMPUTERS ? 'server' : on === shell.client.home() ? here.icon() : 'server', title: shell.t('shell.computers.title'), ariaLabel: shell.t('shell.computers.aria', { name: label }), onOpen: () => openComputers(shell) };
  };
  return (
    <SidebarHead
      computer={computer()}
      homeOn={homeOn()}
      slots={slots()}
      brandLabel={shell.t('shell.brand')}
      searchLabel={shell.t('shell.side.search')}
      viewLabel={shell.t(listOpen() ? 'shell.side.view' : 'shell.side.show')}
      viewPressed={listOpen()}
      manageLabel={shell.t('shell.side.manage')}
      chips={chips()}
      on={on()}
      {...((pk) => (pk ? { pick: pk } : {}))(pick())}
      onBrand={() => shell.navigate({ t: 'home' })}
      onSearch={() => shell.palette.open()}
      onView={() => shell.sidebar.toggle()}
      onChip={(id) => props.setFilter(id)}
      onSwitch={() => openSwitcher(shell)}
      switchLabel={`${shell.t('shell.projects.switch')} (${displayCombo('mod+p', shell.platform.apple).join('')})`}
      onReorder={reorder}
      accountAct={{ menu: { items: account.items(), onSelect: account.onSelect } }}
      accountLabel={account.label()}
      {...(me().initial !== undefined && { me: { initial: me().initial as string, state: me().state, ...(me().avatar !== undefined && { avatar: me().avatar as string }) } })}
      // no bare +: Add project…, Manage projects… and Repo history close the switcher "All ▾" opens (docs/ux #11)
    />
  );
}

/** The pick, without ids of projects no longer there (once the projects have loaded). */
function knownPick(value: string, projects: readonly { readonly id: string }[]): string[] {
  return parsePick(value, projects.length > 0 ? projects.map((p) => p.id) : undefined);
}

/** The strip over the list while several projects are picked (project-pick.ts): each a chip with its ×, then
 *  Only <first> and All projects. Shown over any list (the frame's or shell-list's). */
export function PickStripView(props: { readonly filter: () => string; readonly setFilter: (v: string) => void }): JSX.Element {
  const shell = useShell();
  const projects = useProjects(shell, usePluginClient('shell').home).list;
  const picked = createMemo(() => pickedProjects(knownPick(props.filter(), projects()), projects()));
  return (
    <Show when={picked().length > 1}>
      <PickStrip
        label={shell.t('shell.pick.strip')}
        chips={picked().map((p) => ({ id: p.id, name: p.name, color: p.color || 'slate', title: p.path, removeLabel: shell.t('shell.pick.leave', { name: p.name }) }))}
        onRemove={(id) => props.setFilter(pickValue(pickRemove(picked().map((p) => p.id), id)))}
        only={{ label: shell.t('shell.pick.only', { name: picked()[0]?.name ?? '' }), onClick: () => props.setFilter(pickValue(pickOnly(picked()[0]?.id ?? 'all'))) }}
        all={{ label: shell.t('shell.side.all'), onClick: () => props.setFilter('all') }}
      />
    </Show>
  );
}

/** v1 .split-top: New task and My Stuff, over whichever list fills `shell.sidebar:list` (the frame's own, or
 *  shell-list's): it's the frame's, so a list plugin never takes New task away (at 600 px it was the only way in). */
export function SidebarTop(props: { readonly onNew: () => void; readonly homeOn: boolean }): JSX.Element {
  const shell = useShell();
  const nav = useSignal(shell.nav);
  return (
    <SplitTop
      newLabel={shell.t('shell.side.new')}
      newTitle={shell.t('shell.side.newTitle')}
      stuffLabel={shell.t('shell.side.stuff')}
      on={props.homeOn}
      stuffOn={(() => {
        const m = nav().main;
        return m.kind === 'view' && String(m.id) === 'stuff.page';
      })()}
      onNew={props.onNew}
      onStuff={() => shell.navigate({ t: 'openView', id: 'stuff.page' as never, params: null as Json })}
    />
  );
}

export function SidebarList(props: { readonly onNew: () => void; readonly homeOn: boolean; readonly filter: () => string }): JSX.Element {
  const shell = useShell();
  const here = useHere();
  const sessions = useSessions();
  const nav = useSignal(shell.nav);
  const now = useNow(shell.client);
  const [folded, setFolded] = createSignal<ReadonlySet<SectionId>>(new Set());
  const harnesses = useSignal(shell.harnesses());
  const selected = useSignal(() => shell.selection.get());
  const open = createMemo(() => {
    const m = nav().main;
    return m.kind === 'task' ? m.id : undefined;
  });
  const titleOf = (id: string): string => sessions().find((s) => s.id === id)?.title ?? '';
  const projects = useProjects(shell, usePluginClient('shell').home).list;
  // a crew member picked on the rail: the list holds only their jobs (v1 crew.js inPick)
  const crew = useSignal(shell.client.entities.list<CrewRow>('crew_member'));
  const picked = createMemo(() => {
    const m = nav().main;
    if (m.kind !== 'crew') return undefined;
    // the page is keyed `<peer>:<id>`, a member's row by its bare id
    const id = String(m.id);
    const bare = id.slice(id.indexOf(':') + 1);
    return crew().find((c) => c.id === id || c.id === bare);
  });
  // newest first within a section, as v1's feed
  const bySection = createMemo(() => {
    const out = new Map<SectionId, SessionRow[]>();
    const who = picked()?.id;
    const ids = knownPick(props.filter(), projects());
    const shown = sessions().filter((s) => pickHas(ids, projectOfRow(s, projects())?.id) && (!who || crewOf(s) === who));
    for (const s of shown.sort((a, b) => b.updated_at - a.updated_at)) {
      const k = sectionOf(s);
      out.set(k, [...(out.get(k) ?? []), s]);
    }
    return out;
  });
  const toggle = (id: SectionId): void => {
    const next = new Set(folded());
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setFolded(next);
  };
  return (
    <>
      <Feed>
        <For each={SECTIONS.filter((id) => (bySection().get(id)?.length ?? 0) > 0)}>
          {(id) => (
            <FeedSection id={id} name={shell.t(`shell.section.${id}`)} count={bySection().get(id)?.length ?? 0} collapsed={folded().has(id)} onToggle={() => toggle(id)}>
              <For each={bySection().get(id) ?? []}>
                {(s) => {
                  // the third line names a coding agent: only one a harness plugin describes
                  const h = harnesses().some((x) => x.id === s.agent) ? harnessOf(harnesses(), s.agent) : undefined;
                  const where = here.name();
                  const goal = goalOf(s, shell.t);
                  const overlap = overlapOf(s, titleOf, shell.t);
                  // Auto picked this run: the logo's dot, and why in its tip (v1 runners.js modelBadge)
                  const route = routeOf(s.body);
                  return (
                    <TaskCard
                      slots={rowSlots(s.id, () => s as never)}
                      id={s.id}
                      title={s.title}
                      sub={s.summary.live || s.summary.preview}
                      color={String(s.summary['color'] ?? 'slate')}
                      unread={s.unread}
                      open={open() === s.id}
                      selected={selected() === s.id}
                      done={s.state === 'done'}
                      status={statusOf(s, shell.t)}
                      {...(goal && { goal })}
                      {...(overlap && { overlap })}
                      {...(h && {
                        run: {
                          harness: h.name,
                          where,
                          // the model it ran on, else what its Default runs (one name everywhere), else "Default model"
                          title: route
                            ? routeTip(route, shell.t)
                            : shell.t('shell.card.runTitle', {
                                harness: h.name,
                                model: runModelWord(String(s.summary.model || ''), defaultModelName(shell.client, ownerOf(sessionKey(untrack(shell.client.entities.keys(SESSION)), s.id) as EntityKey), s.agent, String(s.summary.cwd ?? '') || undefined), shell.t('shell.task.defaultModel')),
                                where,
                              }),
                          routed: !!route,
                          ...(h.logo && { logo: h.logo }),
                        },
                      })}
                      time={shortAgo(s.updated_at, now())}
                      timeTitle={longTime(s.updated_at)}
                      laterLabel={shell.t('shell.card.later')}
                      laterOn={isSaved(s.summary['forLater'])}
                      onDone={() => void shell.client.commands.run(setState, { id: s.id, state: 'done' }).catch(() => undefined)}
                      onLater={() => void shell.client.commands.run(forLater, { id: s.id, forLater: !isSaved(s.summary['forLater']) }).catch(() => undefined)}
                      doneLabel={shell.t('shell.card.done')}
                      onOpen={() => {
                        shell.selection.set(s.id);
                        if (s.unread) void shell.client.commands.run(markRead, { id: s.id, unread: false }).catch(() => undefined);
                        shell.navigate({ t: 'openTask', id: sessionKey(untrack(shell.client.entities.keys(SESSION)), s.id) as never });
                      }}
                    />
                  );
                }}
              </For>
            </FeedSection>
          )}
        </For>
      </Feed>
      <Show when={picked() && bySection().size === 0}>
        <EmptyState title={shell.t('shell.crew.none', { name: picked()?.name ?? '' })} compact part="sidebar.crew.none" />
      </Show>
    </>
  );
}

/** The task list's shape while its code loads (QA round 1 #12: blank under New task for up to a second): a few rows,
 *  each its title and its harness line, where the rows will be. */
export function ListLoading(): JSX.Element {
  return (
    <Stack gap="7" part="sidebar.loading">
      {[0, 1, 2, 3, 4, 5].map(() => (
        <Stack gap="3">
          <Skeleton width="2/3" />
          <Skeleton width="1/2" />
        </Stack>
      ))}
    </Stack>
  );
}
