// The side panel beside the open task (v1 features/panel.js): its head (Changes · PR · Shell, then close) and
// the pane. Changes: every file the work changed since it left its base, committed or not, from git
// (`git.changes`, the worktree's base, else HEAD); a folder that isn't a repo shows the turns' own edits. PR
// (PrPane.tsx, `ship.details`) and Shell (ShellPane.tsx, wisp.terminal).
import { Match, Show, Switch, createEffect, createMemo, createResource, createRoot, createSignal, onCleanup, onMount, untrack, type JSX } from 'solid-js';
import { Dynamic } from 'solid-js/web';
import { defineCommand, readonly, entityKey, type EntityKey } from '@wisp/client';
import { IconButton, type IconName, Icon, Inline, Menu, Stack, Tabs, Text, afterNextPaint, useAfterPaint, useDerived, useSignal } from '@wisp/ui';
import { editorUrl } from '../editor.ts';
import { TAB_VIEW, TERM_VIEW, WATCH_VIEW, isTui, pickedTab, tabKey, type SidePane } from '../open-task.ts';
import { showsPlace } from './place.ts';
import { useShell } from '../shell.ts';
import { useTask } from '../task-context.tsx';
import { PrPane } from './PrPane.tsx';
import { ShellPane, askShellFocus, shellWalkedTo, walksTabs } from './ShellPane.tsx';
import { ChangesPane } from './ChangesPane.tsx';
import { changeStat, setChangeStat } from './changes-stat.ts';
import { changesOf, outsideFolder, relative, type ChangeTurn } from './changes.ts';
import { sideStrip, tabCount } from './strip.ts';
import { ofTask, worktreeDir } from './changes.ts';
import { FilePane } from './FilePane.tsx';
import { PreviewPane } from './PreviewPane.tsx';
import { WatchPane } from './WatchPane.tsx';

const bare = (k: string): string => k.slice(k.indexOf(':') + 1);
/** How long the ⋯ shows its check after a copy (as the sign-in Copy, SettingsSignIn's COPIED_MS). */
const COPIED_MS = 1500;
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
/** Background Changes tab file count while another pane is open: stats only, no stage reads or diff rows. */
const readChangesCount = defineCommand<
  { key: EntityKey; path: string; base: string },
  { readonly stats?: readonly unknown[]; readonly body?: { readonly kind: 'inline'; readonly files: readonly unknown[] } | { readonly kind: 'blob' } } | null
>({
  id: 'git.changes',
  owner: (a) => a.key,
  wire: (a) => ({ path: a.path, base: a.base, stageState: false, statsOnly: true }),
  optimistic: 'none',
  quiet: true,
  pending: 'the Changes tab count',
});


/** Until when a panel that mounts takes the keyboard focus onto its open tab: an arrow key (or Enter) on the strip opened
 * a pane whose panel is drawn afresh (Thread, a pinned app: a `view:` side), and the focus went with the old strip (audit
 * E #5: → reached Thread, then the focus fell to the page and the next → did nothing). */
const [refocusUntil, setRefocusUntil] = createRoot(() => createSignal(0));
const REFOCUS_MS = 1000;

export function SidePanel(props: { readonly pane: SidePane; readonly file?: string }): JSX.Element {
  const shell = useShell();
  const task = useTask();
  const t = shell.t;
  // the panel's head and tabs paint in the press's frame; its body (a pane's rows, a terminal) is built in the frame
  // after, and so is the next tab's body on a tab switch, the tab already lit (perf r2 #8: ] and a tab ~100 ms at 4×)
  const [opened, setOpened] = createSignal(typeof requestAnimationFrame !== 'function');
  if (!opened()) onCleanup(afterNextPaint(() => setOpened(true)));
  const body = useAfterPaint(
    () => ({ pane: props.pane, file: props.file }),
    (a, b) => a.pane === b.pane && a.file === b.file,
  );
  const place = (): { where: string; remote: boolean; branch: string; dirty: number; dir: string } | undefined => {
    const r = task.task() as
      | {
          cwd?: string;
          ext?: { 'wisp.worktrees'?: { path?: string } };
          body?: { cwd?: string; ext?: { 'wisp.worktrees'?: { path?: string } } };
          summary?: { cwd?: string; branch?: string; machine?: { name?: string } | null; toShip?: { dirty?: number } | null; worktree?: { branch?: string; path?: string } | null };
        }
      | undefined;
    const sum = r?.summary;
    if (!sum) return undefined;
    // a synced row keeps its folder under `body` (audit A #19: read from the top, it was always empty, and the ⋯ had
    // only Copy branch name); a flat row (a test's, an optimistic one) at the top
    const dir = r?.body?.ext?.['wisp.worktrees']?.path ?? r?.ext?.['wisp.worktrees']?.path ?? sum.worktree?.path ?? r?.body?.cwd ?? r?.cwd ?? sum.cwd ?? '';
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
  // a copy says so on the ⋯ itself: a check for a moment, no toast (owner: toasts are for errors; the sign-in Copy's way)
  const [copied, setCopied] = createSignal(false);
  let uncheck = (): void => {};
  onCleanup(() => uncheck());
  const copy = (text: string): void => {
    setCopied(true);
    uncheck();
    uncheck = task.client.scope.timeout(COPIED_MS, () => setCopied(false));
    void Promise.resolve(shell.platform.copy(text)).catch(() => (uncheck(), setCopied(false)));
  };
  const onPlace = (pl: { branch: string; dir: string }, v: string): void => {
    if (v === 'path') copy(pl.dir);
    if (v === 'branch') copy(pl.branch);
    if (v === 'editor') {
      const url = editorUrl(String(editor()?.value ?? ''), pl.dir);
      if (url) shell.platform.openExternal(url);
    }
    if (v === 'app') void task.client.run(openApp, { key: task.key as EntityKey, cwd: pl.dir }).catch(() => undefined);
  };
  // the last key pressed in the panel (a press of the pointer clears it): an arrow on the strip walks it
  let lastKey = '';
  const open = (pane: string): void => {
    // the Shell's tab pressed while it shows: the keys go to the terminal (audit F tasks #1); another pane keeps them on
    // its tab, so the arrow keys go on along the strip
    if ((pane === 'shell' || pane === 'term') && props.pane === pane) return askShellFocus();
    // walked onto by an arrow key: the shell starts, the keys stay on the strip (audit G tasks #3)
    if ((pane === 'shell' || pane === 'term') && walksTabs(lastKey) && typeof document !== 'undefined' && document.activeElement?.closest("[data-part~='panel.tabs']")) shellWalkedTo();
    if (typeof document !== 'undefined' && document.activeElement?.closest("[data-part~='panel.tabs']") && pane !== 'shell' && pane !== 'term') setRefocusUntil(Date.now() + REFOCUS_MS);
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
  // other plugins' tabs, after the panel's own (v1 panel.js: an app from My Stuff pinned to tasks, "pane:<id>")
  // (a client computed: what the tabs read are the client's signals, the pins setting and the items)
  const defs = shell.sideTabs?.();
  const extra = useDerived(() => (defs?.() ?? []).flatMap((d) => d.tabs({ client: d.client, task: task.key }).map((x) => ({ ...x, key: tabKey(d.plugin, d.id, x.value), def: d }))));
  const picked = useSignal(readonly(pickedTab));
  const shown = createMemo(() => (props.pane === 'tab' ? extra().find((x) => x.key === picked()) : undefined));
  // Changes counts its files (the Changes pane's read, else one of the panel's own while another pane is open), PR names
  // its number and checks (UX 2026-11-18 #5's counts, kept by the owner's 2026-11-19 decision)
  type Raw = { cwd?: string; summary?: { pr?: { number: number; checks?: { pass?: number; fail?: number; pending?: number } } | null; worktree?: { baseRef?: string; base?: string; path?: string; cwd?: string } | null }; body?: { cwd?: string; ext?: Record<string, { baseRef?: string; base?: string; path?: string; cwd?: string } | undefined> } };
  const raw = (): Raw | undefined => task.task() as Raw | undefined;
  const wt = () => raw()?.body?.ext?.['wisp.worktrees'] ?? raw()?.summary?.worktree ?? undefined;
  // the folder git diffs: the task's worktree only. A task in place (or a scratch folder) shows the edits wisp saw, as
  // v1's GET /sessions/:id/changes did: a diff of the project folder holds the user's own work too (audit 2026-11-19:
  // an in-place task in the shared checkout counted 198 files, every session's)
  const gitDir = (): string => worktreeDir(wt());
  const folder = (): string => String(wt()?.path ?? wt()?.cwd ?? raw()?.body?.cwd ?? raw()?.cwd ?? '');
  // a folder git can't read (no repo, too big a diff): the files the task's turns edited, as Changes then lists them
  const turns = useSignal(task.client.entities.list<ChangeTurn>('turn'));
  const taskTurns = createMemo(
    () => turns().filter((x) => ofTask(x.session_id, task.id)),
    undefined,
    { equals: (a, b) => a.length === b.length && a.every((x, i) => x === b[i]) },
  );
  const edited = createMemo(() => [...new Set(taskTurns().flatMap((x) => changesOf(x).map((c) => relative(c.file, folder()))))]);
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
        // the tab's count is a look in the background: its refusal is no toast (a phone since UX 2026-11-18 phone #14;
        // the browser too since the audit of 2026-11-19: a worktree cleaned up after the task toasted "no such folder:
        // /Users/…" on every open); the Changes pane says it in words
        .run(readChangesCount, { key: task.key as EntityKey, path, base })
        .then((g) => {
          if (!g || untrack(() => props.pane) === 'changes') return;
          const n = g.stats ? g.stats.length : g.body?.kind === 'inline' ? g.body.files.length : undefined;
          if (n !== undefined) setChangeStat(task.key, { files: n + untrack(outside) });
        })
        .catch(() => undefined);
    },
  );
  // what isn't committed counts only in a worktree (in place it's the project folder's, the user's own work too)
  const files = (): number => changeStat(task.key)?.files ?? Math.max(touched(), gitDir() ? (place()?.dirty ?? 0) : 0);
  const pr = () => raw()?.summary?.pr ?? undefined;
  // owner, 2026-11-19: every tab in the strip by default, labelled, in v1's fixed order (strip.ts)
  const order = (): string[] => sideStrip({ term: isTui(task.task()) || props.pane === 'term', watch: watched(), extras: extra().map((x) => x.key) });
  const tabOf = (p: string) => {
    const x = extra().find((e) => e.key === p);
    // last in the strip, a pinned tab goes to its icon first when the panel runs short (Tabs `fit`), so the panel's own keep
    // their labels (audit 2026-11-19: "Cha…", "#…", "S…" beside a whole "Token budget dash…")
    if (x) return { value: x.key, label: x.label, icon: (x.icon ?? 'app') as IconName, minor: true, ...(x.title !== undefined && { title: x.title }) };
    const n = p === 'changes' ? files() : 0;
    const prWords = p === 'pr' ? tabCount.pr(pr(), t('shell-task.side.pr')) : undefined;
    return {
      value: p,
      label: prWords ?? t(`shell-task.side.${p}`),
      // Watch gives way with the pinned tabs, before the row tightens and Preview goes (owner follow-up, 2026-11-19)
      ...(p === 'watch' && { minor: true }),
      ...(n > 0 && { count: n }),
      icon: (p === 'changes' ? 'file' : p === 'pr' ? 'merge' : p === 'preview' ? 'web' : p === 'watch' ? 'eye' : 'terminal') as IconName,
    };
  };
  // its tab gone (unpinned while open, or a reload forgot which): Changes, as v1 (panel.js tabOf)
  createEffect(() => {
    if (props.pane === 'tab' && !shown()) shell.navigate({ t: 'openSide', side: { kind: 'changes', id: task.key as never } });
  });
  let head: HTMLElement | undefined;
  onMount(() => {
    const el = head;
    if (!el) return;
    const key = (e: KeyboardEvent): void => void (lastKey = e.key);
    const press = (): void => void (lastKey = '');
    el.addEventListener('keydown', key, true);
    el.addEventListener('pointerdown', press, true);
    onCleanup(() => (el.removeEventListener('keydown', key, true), el.removeEventListener('pointerdown', press, true)));
  });
  onMount(() => {
    if (Date.now() > untrack(refocusUntil)) return;
    setRefocusUntil(0);
    afterNextPaint(() => head?.querySelector<HTMLElement>("[data-part~='panel.tabs'] [data-kit='tab'][data-selected]")?.focus({ preventScroll: true }));
  });
  return (
    <Stack gap="0" part={'side.panel panel' as `${string}.${string}`} grow ref={(el) => (head = el)}>
      <Inline gap="2" align="center" part="side.head">
        <Tabs part="panel.tabs" variant="pill" fit aria-label={t('shell-task.side.label')} value={shown()?.key ?? props.pane} onChange={open} items={order().map(tabOf)} />
        {/* where the work is: copy it, open it elsewhere (v1's place row ⋯, in the head over every pane) */}
        <Show when={place()}>
          {(pl) => (
            <Show when={placeItems(pl()).length > 0}>
              <Menu
                part="panel.place.more"
                trigger={{ icon: copied() ? 'check' : 'dots', 'aria-label': copied() ? t('shell-task.side.copied') : t('shell-task.side.more'), variant: 'ghost', size: 'sm' }}
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
      {/* Changes runs edge to edge (v1 #sidePanel .cx), and so does Shell in the browser (v1 .pn-shell: the terminal's
          colour to the panel's edges, audit 2026-11-19: a dark box in a white margin read as an empty area); PR keeps the
          body's padding, and so does a phone's Shell (its look sets its own) */}
      <Stack gap="0" grow part={(body().pane === 'changes' && !body().file) || body().pane === 'preview' || body().pane === 'watch' || body().pane === 'tab' || (!shell.platform.phone && (body().pane === 'shell' || body().pane === 'term')) ? 'side.body side.flush' : 'side.body'}>
        <Show when={opened()}>
        <Switch>
          <Match when={body().pane === 'changes' && body().file}>{(f) => <FilePane path={f()} />}</Match>
          <Match when={body().pane === 'changes'}>
            <ChangesPane />
          </Match>
          <Match when={body().pane === 'pr'}>
            <PrPane />
          </Match>
          <Match when={body().pane === 'shell'}>
            <ShellPane kind="shell" />
          </Match>
          <Match when={body().pane === 'preview'}>
            <PreviewPane />
          </Match>
          <Match when={body().pane === 'watch'}>
            <WatchPane />
          </Match>
          <Match when={body().pane === 'term'}>
            <ShellPane kind="takeover" />
          </Match>
          <Match when={body().pane === 'tab' && shown()}>{(x) => <Dynamic component={x().def.component} client={x().def.client} task={task.key} value={x().value} />}</Match>
        </Switch>
        </Show>
      </Stack>
    </Stack>
  );
}
