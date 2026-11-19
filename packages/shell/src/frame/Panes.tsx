// The three panes. Each is one area a plugin fills (the contract's shell.sidebar:list,
// shell.screen:*, shell.side:*), or a plugin view named by navigation. Until a plugin fills an area
// the shell shows a small built-in: the list, a task's activity, a note on Home. Screens are keyed
// by what they show, so opening another task builds its screen afresh (its scopes, its
// subscriptions) and the old one's end with it.

import { Match, Show, Switch, createMemo, createSignal, getOwner, lazy, runWithOwner, untrack, type JSX } from 'solid-js';
import { Dynamic } from 'solid-js/web';
import type { EntityKey, Nav } from '@wisp/client';
import { EmptyState, Skeleton, Stack, useSignal } from '@wisp/ui';
import { useShell } from '../context.tsx';
import type { ExtensionProps, ResolvedExtension } from '../contract.ts';
import { Area, Slot, useExtensions, useViewClient } from '../slot.tsx';
import { componentIds, sideKey, type DrawnBy } from './side-key.ts';
import { SESSION, type SessionRow } from '../rows.ts';
import { FallbackList } from './FallbackList.tsx';
import { backOf } from './rows-view.ts';
import { PeerBanner } from './PeerBanner.tsx';
import { stackView } from './stack-page.ts';
import { PaneBoundary } from './PaneBoundary.tsx';

/** lazy: the task page's own code; App warms it while the list sits idle (warm-task.ts) */
export const TaskView = lazy(() => import('./TaskView.tsx'));
/** Home (the prompt box, Needs you, the panes): its own chunk (ADR-0105), so a phone's first screen (the list) never
 *  loads it; the desktop asks for it as the app starts (App.tsx), so it comes in alongside the entry. */
const home: { m?: typeof import('./HomeView.tsx'); loading?: Promise<typeof import('./HomeView.tsx')> | undefined } = {};
export const loadHome = (): Promise<typeof import('./HomeView.tsx')> =>
  (home.loading ??= import('./HomeView.tsx').then(
    (m) => (home.m = m),
    (err: unknown) => {
      home.loading = undefined;
      throw err;
    },
  ));

/** whether Home's chunk is in (the frame then draws at once) */
export const homeLoaded = (): boolean => home.m !== undefined;

/** Home once its chunk is in (the desktop's frame waits for it: App.tsx), its skeleton until then (a phone's New task
 *  pressed before the chunk landed) */
function HomeView(props: { readonly onBack?: () => void }): JSX.Element {
  const [m, setM] = createSignal(home.m);
  if (!m()) void loadHome().then(setM, () => undefined);
  return (
    <Show when={m()} keyed fallback={<Stack gap="3" part="shell.home.loading"><Skeleton shape="block" height="bar" /><Skeleton shape="line" width="2/3" /></Stack>}>
      {(mod) => <mod.HomeView {...props} />}
    </Show>
  );
}

type SessionProp = NonNullable<ExtensionProps['session']>;

function useSession(id: () => string | undefined): SessionProp {
  const shell = useShell();
  const row = createMemo(() => {
    const key = id();
    return key === undefined ? undefined : shell.client.entities.row<SessionRow>(SESSION, key as EntityKey);
  });
  return () => row()?.() as ReturnType<SessionProp>;
}

export function ListPane(): JSX.Element {
  return (
    <Stack gap="0" grow part="shell.list.pane">
      <PeerBanner />
      <PaneBoundary pane="list">
        <Slot input="shell.list:top" layout="stack" />
        <Area input="shell.sidebar:list" fallback={<FallbackList />} />
      </PaneBoundary>
    </Stack>
  );
}

function ViewOf(props: { readonly kind: string; readonly params: unknown }): JSX.Element {
  const shell = useShell();
  const def = createMemo(() => shell.view(props.kind));
  return (
    <Show when={def()} keyed fallback={<EmptyState icon="info" title={shell.t('shell.view.missing')} compact />}>
      {(d) => {
        const client = useViewClient(d.kind);
        return <Dynamic component={d.component} client={client} params={d.parse(props.params)} />;
      }}
    </Show>
  );
}

const mainKey = (n: Nav): string => (n.main.kind === 'home' ? 'home' : n.main.kind === 'view' ? `view:${n.main.id}:${JSON.stringify(n.main.params)}` : `${n.main.kind}:${n.main.id}`);

/** `launcher`: New task is open as a phone's page. While the list is the page Home isn't built (stack-page.ts), so
 *  opening a task doesn't tear down a hidden prompt box first and Back doesn't build one (F-phone-8: 58 ms of a 93 ms
 *  tap on a 4× slowed phone). Worked out from this pane's own nav: a prop derived from App's would change a step
 *  ahead of it and build Home for one pass on every tap. */
export function MainPane(props: { readonly onBack?: () => void; readonly launcher?: boolean }): JSX.Element {
  const shell = useShell();
  const nav = useSignal(shell.nav);
  const idleHome = (): boolean => stackView(nav(), props.launcher === true).idleHome;
  // the layout is part of the key (Back shows in a stack); nothing else of nav may rebuild the page: a side
  // panel opening must keep the task's open steps, its scroll and the focus (v1's panel did)
  const key = createMemo(() => `${mainKey(nav())}|${nav().layout}${idleHome() ? '|idle' : ''}`);
  return (
    <Show when={key()} keyed>
      {(_k) => <PaneBoundary pane="main">{mainPage()}</PaneBoundary>}
    </Show>
  );
  function mainPage(): JSX.Element {
    const now = untrack(nav);
    const main = now.main;
    switch (main.kind) {
      case 'home':
        if (untrack(idleHome)) return null;
        return <Area input="shell.screen:home" fallback={<HomeView {...(now.layout === 'stack' && props.onBack && { onBack: props.onBack })} />} />;
      case 'task': {
        // v1 #773: Back from a subagent's run opens its lead; anything else goes back through history
        const goBack = (id: string, row: { parent_id?: string | null } | undefined): void => {
          const lead = backOf(id, row);
          shell.navigate(lead ? { t: 'openTask', id: lead as never } : { t: 'back' });
        };
        const id = main.id;
        const session = useSession(() => id);
        return (
          <Area
            input="shell.screen:task"
            id={id}
            session={session}
            fallback={<TaskView id={id} {...(now.layout === 'stack' && props.onBack && { onBack: () => goBack(id, session() as { parent_id?: string | null } | undefined) })} />}
          />
        );
      }
      case 'crew':
        return <Area input="shell.screen:crew" id={main.id} fallback={<EmptyState icon="crew" title={shell.t('shell.view.missing')} compact />} />;
      case 'view':
        return <ViewOf kind={main.id} params={main.params} />;
    }
  }
}

export function SidePane(): JSX.Element {
  const shell = useShell();
  const nav = useSignal(shell.nav);
  const side = createMemo(() => nav().side);
  // one panel for a task's tabs drawn by the same code: a tab switch swaps its body, not the whole panel (side-key.ts)
  const owner = getOwner();
  const areas = new Map<string, () => readonly ResolvedExtension[]>();
  const drawnBy = (kind: string): DrawnBy | undefined => {
    let list = areas.get(kind);
    if (!list) areas.set(kind, (list = runWithOwner(owner, () => useExtensions(`shell.side:${kind}`)) ?? (() => [])));
    const ext = list().at(-1);
    return ext && { plugin: ext.plugin, component: ext.def.component as object, config: ext.config };
  };
  const idOf = componentIds();
  const key = createMemo(() => sideKey(side(), drawnBy, idOf));
  return (
    <Show when={key()} keyed>
      {(_k) => {
        const s = side();
        return (
          <PaneBoundary pane="side">
            <Switch>
              <Match when={s.kind === 'view' && s}>{(v) => <ViewOf kind={v().id} params={null} />}</Match>
              <Match when={s.kind !== 'view' && s.kind !== 'none' && s}>
                {(p) => {
                  const id = p().id;
                  const session = useSession(() => id);
                  return <Area input={`shell.side:${p().kind}`} id={id} session={session} fallback={<EmptyState icon="panel" title={shell.t('shell.view.missing')} compact />} />;
                }}
              </Match>
            </Switch>
          </PaneBoundary>
        );
      }}
    </Show>
  );
}
