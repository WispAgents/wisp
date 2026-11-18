// The side panel beside the open task (v1 features/panel.js): its head (Changes · PR · Shell, then close) and
// the pane. Changes: every file the work changed since it left its base, committed or not, from git
// (`git.changes`, the worktree's base, else HEAD); a folder that isn't a repo shows the turns' own edits. PR
// (PrPane.tsx, `ship.details`) and Shell (ShellPane.tsx, wisp.terminal).
import { Match, Show, Switch, createEffect, createMemo, createResource, untrack, type JSX } from 'solid-js';
import { Dynamic } from 'solid-js/web';
import { computed, defineCommand, readonly, entityKey, type EntityKey } from '@wisp/client';
import { Box, IconButton, type IconName, Icon, Inline, Menu, Stack, Tabs, Text, useSignal } from '@wisp/ui';
import { editorUrl } from '../editor.ts';
import { TAB_VIEW, TERM_VIEW, WATCH_VIEW, isTui, pickedTab, tabKey, type SidePane } from '../open-task.ts';
import { showsPlace } from './place.ts';
import { useShell } from '../shell.ts';
import { useTask } from '../task-context.tsx';
import { PrPane } from './PrPane.tsx';
import { ShellPane } from './ShellPane.tsx';
import { ChangesPane, read as readChanges, readHere as readChangesQuiet } from './ChangesPane.tsx';
import { changeStat, setChangeStat } from './changes-stat.ts';
import { hasPr } from './pr.ts';
import { changesOf, outsideFolder, relative, type ChangeTurn } from './changes.ts';
import { sideStrip, tabCount } from './strip.ts';
import { FilePane } from './FilePane.tsx';
import { PreviewPane } from './PreviewPane.tsx';
import { WatchPane } from './WatchPane.tsx';

const bare = (k: string): string => k.slice(k.indexOf(':') + 1);
/** whether a terminal app can open here for this page (wisp.terminal) */
const shellInfo = defineCommand<{ key: EntityKey; id: string }, { app: boolean }>({
  id: 'terminal.shell.info',
  owner: (a) => a.key,
  wire: (a) => ({ id: a.id }),
  optimistic: 'none',
  pending: 'the place menu',
});
/** v1 POST /sessions/:id/open-shell: the folder in the terminal app Settings names */
const openApp = defineCommand<{ key: EntityKey; cwd: string }, { ok: boolean }>({
  id: 'terminal.app.open',
  owner: (a) => a.key,
  wire: (a) => ({ cwd: a.cwd, argv: [] }),
  optimistic: 'none',
  pending: 'Open in terminal app',
});

const PANES: readonly SidePane[] = ['changes', 'pr', 'shell', 'preview'];
/** whether the task's app is running for Preview (wisp.terminal) */
const runInfo = defineCommand<{ key: EntityKey; id: string }, { open: boolean }>({
  id: 'terminal.run.info',
  owner: (a) => a.key,
  wire: (a) => ({ id: a.id }),
  optimistic: 'none',
  pending: 'the side panel\'s Preview tab',
});

export function SidePanel(props: { readonly pane: SidePane; readonly file?: string }): JSX.Element {
  const shell = useShell();
  const task = useTask();
  const t = shell.t;
  const place = (): { where: string; remote: boolean; branch: string; dirty: number; dir: string } | undefined => {
    const r = task.task() as
      | {
          cwd?: string;
          ext?: { 'wisp.worktrees'?: { path?: string } };
          summary?: { branch?: string; machine?: { name?: string } | null; toShip?: { dirty?: number } | null; worktree?: { branch?: string; path?: string } | null };
        }
      | undefined;
    const sum = r?.summary;
    if (!sum) return undefined;
    const dir = r?.ext?.['wisp.worktrees']?.path ?? sum.worktree?.path ?? r?.cwd ?? '';
    return { where: sum.machine?.name ?? t('shell-task.side.here'), remote: !!sum.machine, branch: sum.worktree?.branch ?? sum.branch ?? '', dirty: sum.toShip?.dirty ?? 0, dir };
  };
  // the place row's ⋯ (v1 panel.js placeMenu): copy where the work is, open it in the editor Settings names (its URL
  // scheme), or in the terminal app (wisp.terminal's terminal.app.open; only where one can open: this computer's desktop)
  const editor = useSignal(task.client.entities.row<{ value?: unknown }>('setting', entityKey(task.client.home, 'settings.editor')));
  const [app] = createResource(() =>
    task.client.run(shellInfo, { key: task.key as EntityKey, id: bare(task.key) }).then(
      (i) => i.app,
      () => false,
    ),
  );
  const placeItems = (pl: { branch: string; dir: string; remote: boolean }) => [
    ...(pl.dir ? [{ value: 'path', label: t('shell-task.side.copyPath'), icon: 'copy' as const }] : []),
    ...(pl.branch ? [{ value: 'branch', label: t('shell-task.side.copyBranch'), icon: 'branch' as const }] : []),
    ...(pl.dir && !pl.remote && editorUrl(String(editor()?.value ?? ''), pl.dir) ? [{ value: 'editor', label: t('shell-task.side.openEditor'), icon: 'folder' as const }] : []),
    ...(pl.dir && !pl.remote && app() ? [{ value: 'app', label: t('shell-task.side.openApp'), icon: 'terminal' as const }] : []),
  ];
  const onPlace = (pl: { branch: string; dir: string }, v: string): void => {
    if (v === 'path') void shell.platform.copy(pl.dir);
    if (v === 'branch') void shell.platform.copy(pl.branch);
    if (v === 'editor') {
      const url = editorUrl(String(editor()?.value ?? ''), pl.dir);
      if (url) shell.platform.openExternal(url);
    }
    if (v === 'app') void task.client.run(openApp, { key: task.key as EntityKey, cwd: pl.dir }).catch(() => undefined);
  };
  const open = (pane: string): void => {
    if (pane === 'changes' || pane === 'pr' || pane === 'shell' || pane === 'preview') shell.navigate({ t: 'openSide', side: { kind: pane, id: task.key as never } });
    if (pane === 'term') shell.navigate({ t: 'openSide', side: { kind: 'view', id: TERM_VIEW as never } });
    if (pane === 'watch') shell.navigate({ t: 'openSide', side: { kind: 'view', id: WATCH_VIEW as never } });
    if (extra().some((x) => x.key === pane)) {
      pickedTab(pane);
      shell.navigate({ t: 'openSide', side: { kind: 'view', id: TAB_VIEW as never } });
    }
  };
  // a terminal task's CLI comes first; Watch after the panel's own while the task has one (v1 panel.js tabs)
  const watched = (): boolean => !!(task.task() as { summary?: { watch?: unknown } } | undefined)?.summary?.watch || props.pane === 'watch';
  const panes = (): readonly SidePane[] => [...(isTui(task.task()) || props.pane === 'term' ? (['term'] as const) : []), ...PANES, ...(watched() ? (['watch'] as const) : [])];
  // other plugins' tabs, after the panel's own (v1 panel.js: an app from My Stuff pinned to tasks, "pane:<id>")
  // (a client computed: what the tabs read are the client's signals, the pins setting and the items)
  const defs = shell.sideTabs?.();
  const extra = useSignal(computed(() => (defs?.() ?? []).flatMap((d) => d.tabs({ client: d.client, task: task.key }).map((x) => ({ ...x, key: tabKey(d.plugin, d.id, x.value), def: d })))));
  const picked = useSignal(readonly(pickedTab));
  const shown = createMemo(() => (props.pane === 'tab' ? extra().find((x) => x.key === picked()) : undefined));
  // UX 2026-11-18 #5: the strip holds the tabs with something in them; Changes counts its files (the Changes pane's
  // read, else one of the panel's own while another pane is open), PR names its number and checks
  type Raw = { cwd?: string; summary?: { pr?: { number: number; checks?: { pass?: number; fail?: number; pending?: number } } | null; worktree?: { baseRef?: string; base?: string; path?: string; cwd?: string } | null }; body?: { cwd?: string; ext?: Record<string, { baseRef?: string; base?: string; path?: string; cwd?: string } | undefined> } };
  const raw = (): Raw | undefined => task.task() as Raw | undefined;
  const wt = () => raw()?.body?.ext?.['wisp.worktrees'] ?? raw()?.summary?.worktree ?? undefined;
  const gitDir = (): string => String(wt()?.path ?? wt()?.cwd ?? raw()?.body?.cwd ?? raw()?.cwd ?? '');
  // a folder git can't read (no repo, too big a diff): the files the task's turns edited, as Changes then lists them
  const turns = useSignal(task.client.entities.list<ChangeTurn>('turn'));
  const edited = createMemo(() => [...new Set(turns().filter((x) => x.session_id === task.id).flatMap((x) => changesOf(x).map((c) => relative(c.file, gitDir()))))]);
  const touched = (): number => edited().length;
  // what wisp saw edited outside the folder git reads: the Changes pane lists those beside git's, so the count does too
  const outside = (): number => outsideFolder(edited().map((path) => ({ path }))).length;
  // read again when the work can have moved: the task's state, what's uncommitted or ahead (not on every row patch while
  // it streams); the answer is dropped once Changes is open (its own read is the newer one)
  const moved = (): string => {
    const r = task.task() as { state?: string; summary?: { toShip?: { dirty?: number; ahead?: number } | null } } | undefined;
    return `${r?.state ?? ''}|${r?.summary?.toShip?.dirty ?? 0}|${r?.summary?.toShip?.ahead ?? 0}`;
  };
  createResource(
    () => (props.pane !== 'changes' && gitDir() ? `${gitDir()}\n${wt()?.baseRef || wt()?.base || 'HEAD'}\n${moved()}` : undefined),
    (k) => {
      const [path = '', base = 'HEAD'] = k.split('\n');
      return task.client
        // a phone (UX 2026-11-18 phone #14): the tab's count is a look in the background; its refusal is no toast
        .run(shell.platform.phone ? readChangesQuiet : readChanges, { key: task.key as EntityKey, path, base, reveal: [] })
        .then((g) => g?.body.kind === 'inline' && untrack(() => props.pane) !== 'changes' && setChangeStat(task.key, { files: g.body.files.length + untrack(outside) }))
        .catch(() => undefined);
    },
  );
  const [running] = createResource(() =>
    task.client.run(runInfo, { key: task.key as EntityKey, id: bare(task.key) }).then(
      (i) => i.open,
      () => false,
    ),
  );
  const files = (): number => changeStat(task.key)?.files ?? Math.max(touched(), place()?.dirty ?? 0);
  const pr = () => raw()?.summary?.pr ?? undefined;
  const has = (p: string): boolean =>
    p === 'changes' ? files() > 0 : p === 'pr' ? hasPr(task.task()) : p === 'preview' ? !!running() : !extra().some((x) => x.key === p && x.quiet);
  const order = (): string[] => [...panes(), ...extra().map((x) => x.key)];
  // what had something in it stays in the strip while the panel is open on this task (it never jumps out); a tab only
  // opened from + leaves again when you move on
  const strip = createMemo<{ task: string; joined: readonly string[]; strip: string[]; more: string[] }>((was) => {
    const now = order().filter(has);
    const before = was?.task === task.key ? was.joined.filter((p) => order().includes(p)) : [];
    const joined = [...before, ...now.filter((p) => !before.includes(p))];
    // a phone's sheet (QA r6 #5): every pane a labelled tab, none behind +, as the round-3 sheet had them
    if (shell.platform.phone) return { task: task.key, joined, strip: order(), more: [] };
    return { task: task.key, joined, ...sideStrip({ order: order(), has: new Set(now), current: shown()?.key ?? props.pane, joined }) };
  });
  const tabOf = (p: string) => {
    const x = extra().find((e) => e.key === p);
    if (x) return { value: x.key, label: x.label, icon: (x.icon ?? 'app') as IconName, ...(x.title !== undefined && { title: x.title }) };
    const n = p === 'changes' ? files() : 0;
    const prWords = p === 'pr' ? tabCount.pr(pr()) : undefined;
    return {
      value: p,
      label: prWords ?? t(`shell-task.side.${p}`),
      ...(prWords && { title: t('shell-task.side.pr') }),
      ...(n > 0 && { count: n }),
      icon: (p === 'changes' ? 'file' : p === 'pr' ? 'merge' : p === 'preview' ? 'web' : p === 'watch' ? 'eye' : 'terminal') as IconName,
    };
  };
  // its tab gone (unpinned while open, or a reload forgot which): Changes, as v1 (panel.js tabOf)
  createEffect(() => {
    if (props.pane === 'tab' && !shown()) shell.navigate({ t: 'openSide', side: { kind: 'changes', id: task.key as never } });
  });
  return (
    <Stack gap="0" part={'side.panel panel' as `${string}.${string}`} grow>
      <Inline gap="2" align="center" part="side.head">
        <Tabs
          part="panel.tabs"
          variant="pill"
          aria-label={t('shell-task.side.label')}
          value={shown()?.key ?? props.pane}
          onChange={open}
          items={strip().strip.map(tabOf)}
        />
        <Box grow />
        {/* the tabs with nothing in them yet, one press away (UX 2026-11-18 #5) */}
        <Show when={strip().more.length > 0}>
          <Menu
            part="panel.tabs.more"
            trigger={{ icon: 'plus', 'aria-label': t('shell-task.side.moreTabs'), variant: 'ghost', size: 'sm' }}
            items={strip().more.map((p) => ((x) => ({ value: x.value, label: x.label, icon: x.icon }))(tabOf(p)))}
            onSelect={open}
          />
        </Show>
        {/* where the work is: copy it, open it elsewhere (v1's place row ⋯, in the head over every pane) */}
        <Show when={place()}>
          {(pl) => (
            <Show when={placeItems(pl()).length > 0}>
              <Menu
                part="panel.place.more"
                trigger={{ icon: 'dots', 'aria-label': t('shell-task.side.more'), variant: 'ghost', size: 'sm' }}
                items={placeItems(pl())}
                onSelect={(v) => onPlace(pl(), v)}
              />
            </Show>
          )}
        </Show>
        <IconButton icon="x" label={t('shell-task.side.close')} onClick={() => shell.navigate({ t: 'closeSide' })} />
      </Inline>
      {/* v1 panel.js .pn-place: where the task's work is, its branch, and what isn't committed */}
      <Show when={showsPlace(props.pane, !!place()?.remote) && place()}>
        {(pl) => (
          <Inline gap="2" align="center" part="panel.place">
            <Icon name={pl().remote ? 'server' : 'laptop'} size="sm" tone="faint" />
            <Text size="sm" tone="muted" nowrap>
              {pl().where}
            </Text>
            <Show when={pl().branch}>
              <Text size="sm" tone="faint">
                ·
              </Text>
              <Text size="sm" tone="muted" mono truncate title={pl().branch}>
                {pl().branch}
              </Text>
            </Show>
            <Show when={pl().dirty > 0}>
              <Text size="sm" tone="faint" nowrap>
                {t('shell-task.side.uncommitted', { n: pl().dirty })}
              </Text>
            </Show>
          </Inline>
        )}
      </Show>
      {/* Changes runs edge to edge (v1 #sidePanel .cx); PR and Shell keep the body's padding */}
      <Stack gap="0" grow part={(props.pane === 'changes' && !props.file) || props.pane === 'preview' || props.pane === 'watch' || props.pane === 'tab' ? 'side.body side.flush' : 'side.body'}>
        <Switch>
          <Match when={props.pane === 'changes' && props.file}>{(f) => <FilePane path={f()} />}</Match>
          <Match when={props.pane === 'changes'}>
            <ChangesPane />
          </Match>
          <Match when={props.pane === 'pr'}>
            <PrPane />
          </Match>
          <Match when={props.pane === 'shell'}>
            <ShellPane kind="shell" />
          </Match>
          <Match when={props.pane === 'preview'}>
            <PreviewPane />
          </Match>
          <Match when={props.pane === 'watch'}>
            <WatchPane />
          </Match>
          <Match when={props.pane === 'term'}>
            <ShellPane kind="takeover" />
          </Match>
          <Match when={shown()}>{(x) => <Dynamic component={x().def.component} client={x().def.client} task={task.key} value={x().value} />}</Match>
        </Switch>
      </Stack>
    </Stack>
  );
}
