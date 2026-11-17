// The app: v1's frame (the kit's Stage: crew rail, the sidebar's head and list, the main pane), the
// overlay stack, the toasts and the task switcher. Layout comes from navigation only (nav.layout,
// from the width). On a phone (stack) one page shows: the list, or the main pane when a task, a
// view or New task is open (New task from the list opens Home's prompt box as its own page).

import { createMemo, createSignal, createEffect, lazy, on, onCleanup, onMount, type JSX } from 'solid-js';
import type { Readable } from '@wisp/client';
import { Stage, Toasts, useSignal, type StagePage } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { Area, ClientsProvider, type Clients } from '../slot.tsx';
import { MainPane, SidePane, TaskView } from './Panes.tsx';
import { TASK_PAGE_INPUTS, taskPagePreloads } from './warm-task.ts';
import { Overlays } from './Overlays.tsx';
import { Head, PickStripView, Rail, SidebarList, SidebarTop } from './Sidebar.tsx';
import { crewTint, type TintMember } from './crew-tint.ts';
import { flatRow } from './rows-view.ts';
import { escLeavesLauncher, stackView } from './stack-page.ts';

const Switcher = lazy(() => import('../switcher/Switcher.tsx'));
// the phone apps' destinations: their own chunk, so the browser's first download never carries the navigation bar
const PhoneNav = lazy(() => import('./PhoneNav.tsx'));
// the phone apps' Tasks head (ADR-0065 step 13): the TopAppBar with the computer filter and the project selector, and the FAB
const PhoneHead = lazy(() => import('./PhoneHead.tsx'));
const PhoneListTop = lazy(() => import('./PhoneHead.tsx').then((m) => ({ default: m.PhoneListTop })));

export function App(props: { readonly clients: Clients; readonly recent: Readable<readonly string[]> }): JSX.Element {
  const shell = useShell();
  const nav = useSignal(shell.nav);
  // stack only: Home's prompt box as its own page (v1 body.stack-page), opened from New task
  const [launcher, setLauncher] = createSignal(false);
  // the top bar's project chips; the list (shell-list, or the fallback) follows it
  const picked = useSignal(shell.projectFilter?.value ?? (() => 'all'));
  const filter = (): string => picked() || 'all';
  const setFilter = (id: string): void => shell.projectFilter?.set(id);
  createEffect(
    on(
      () => nav().main.kind,
      (k) => k !== 'home' && setLauncher(false),
    ),
  );
  // the phone list's selection mode ends when the list leaves the screen (a task, New task, another destination)
  createEffect(
    on(
      () => `${nav().main.kind}:${nav().layout}`,
      () => shell.listSelect?.exit(),
      { defer: true },
    ),
  );
  const view = createMemo(() => stackView(nav(), launcher()));
  const page = (): StagePage => view().page;
  const switcher = shell.platform.kind === 'desktop';
  // the phone apps (Android, iPhone): their own head, New task and destinations; a phone's browser keeps v1's
  const phone = shell.platform.phone;
  // the desktop app on a Mac: the page is the title bar (Stage's titlebar, the platform's drag regions)
  const titlebar = shell.platform.titlebar === 'mac' ? 'mac' : undefined;
  onMount(() => onCleanup(shell.platform.dragRegions(document.body)));
  // a phone's New task page: Esc goes back to the list (unless a menu or dialog took the key: defaultPrevented, overlays)
  onMount(() => {
    const onKey = (e: KeyboardEvent): void => {
      const typing = e.target instanceof Element && e.target.closest('input, textarea, select, [contenteditable="true"]') !== null;
      if (escLeavesLauncher(shell.nav(), launcher(), { key: e.key, defaultPrevented: e.defaultPrevented, typing })) setLauncher(false);
    };
    document.addEventListener('keydown', onKey);
    onCleanup(() => document.removeEventListener('keydown', onKey));
  });
  // F-phone-8: the task page's code loads while the list sits idle, so the first tap on a slow phone doesn't wait
  // on the network with nothing on screen
  onMount(() => {
    const warm = (): void => {
      const comps = [TaskView, ...TASK_PAGE_INPUTS.flatMap((i) => shell.extensions(i)().map((e) => e.def.component))];
      for (const run of taskPagePreloads(comps)) void run();
    };
    const idle = window.requestIdleCallback?.bind(window);
    const t = setTimeout(() => (idle ? idle(warm, { timeout: 4000 }) : warm()), 1500);
    onCleanup(() => clearTimeout(t));
  });
  // a crew member picked on the rail tints the list beside it (v1 #768); Settings → General → Crew tint turns it off
  const crew = useSignal(shell.client.entities.list<TintMember>('crew_member'));
  const settings = useSignal(shell.client.entities.list<{ key: string; value: unknown }>('setting'));
  const tint = createMemo(() => {
    const n = nav().main;
    const picked =
      n.kind === 'crew'
        ? crew()
            .map((r) => flatRow<TintMember>(r))
            // the page is keyed `<peer>:<id>`, a member's row by its bare id (the tint never matched)
            .find((m) => m.id === n.id || m.id === String(n.id).slice(String(n.id).indexOf(':') + 1))
        : undefined;
    return crewTint(picked, settings().find((r) => r.key === 'crew.tint')?.value);
  });
  const homeOn = (): boolean => view().homeOn;
  const onNew = (): void => {
    setLauncher(true);
    shell.navigate({ t: 'home' });
    shell.focusTargets.focus('launcher');
  };
  return (
    <ClientsProvider value={props.clients}>
      <Stage
        layout={nav().layout}
        page={page()}
        rail={<Rail />}
        // the phone apps draw the platform's bar and the FAB in place of v1's head and New task row (step 13)
        head={phone ? <PhoneHead filter={filter} /> : <Head filter={filter} setFilter={setFilter} />}
        list={
          <>
            {phone ? <PhoneListTop onNew={onNew} /> : <SidebarTop onNew={onNew} homeOn={homeOn()} />}
            <PickStripView filter={filter} setFilter={setFilter} />
            <Area input="shell.sidebar:list" fallback={<SidebarList onNew={onNew} homeOn={homeOn()} filter={filter} />} />
          </>
        }
        main={<MainPane onBack={() => setLauncher(false)} launcher={launcher()} />}
        // a plain prop, not a spread: a reactive spread rebuilds every prop (the task page too) when the side opens
        side={nav().side.kind !== 'none' ? <SidePane /> : undefined}
        gripLabel={shell.t('shell.side.grip')}
        listBehind={view().listBehind}
        tint={tint()}
        // the phone apps (ADR-0065 step 8): Tasks · Crew · Stuff · Computers; the browser passes none
        nav={phone ? <PhoneNav /> : undefined}
        mainLabel={shell.t('shell.main')}
        sideLabel={shell.t('shell.side')}
        {...(titlebar && { titlebar })}
      />
      <Overlays />
      <Toasts toaster={shell.toast} />
      {switcher ? <Switcher recent={props.recent} /> : null}
    </ClientsProvider>
  );
}
