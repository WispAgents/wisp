// The side panel's Changes (v1 features/changes.js, F-page-21/22): git's diff of the task's work since it left its
// base, committed or not, in the kit's ChangesList. In a worktree a file or a hunk is staged or unstaged, what's
// staged is committed (nothing staged: everything changed), and a hunk or a whole file is reverted. Each answers at
// once (design/feedback.md): the toggle flips, the reverted hunk goes, the committed files go; then git's own read
// comes back. A refusal puts it back as it was, with the error. Without git, the edits wisp saw, per file.
import { Show, createEffect, createMemo, createResource, createSignal, on, onCleanup, untrack, type JSX } from 'solid-js';
import { defineCommand, entityKey, ownerOf, type EntityKey, type SettingRow } from '@wisp/client';
import { Button, ChangesList, CommentCard, Dialog, EmptyState, Inline, isEnvFile, rowKey, useSignal, type ChangeFile, type ChangeRow, type StageState } from '@wisp/ui';
import { openFinder } from './open-file.ts';
import { added, commentStore, isLine, lineKey, lineLabel, placeOf, waiting } from '../comments.ts';
import { isStub, keepTurnsWhole } from '@wisp/shell';
import { useShell } from '../shell.ts';
import { useTask } from '../task-context.tsx';
import { setChangeStat } from './changes-stat.ts';
import { baseName, revertAsk, undoSteps, type UndoStep } from './revert.ts';
import { autoOpenable, changesOf, defaultOpen, envState, filesOf, hasOutsideFolder, isOutsidePath, relative, revealedOf, seenMasked, setRevealed, worktreeDir, ofTask, type ChangeTurn } from './changes.ts';
import '../i18n/lazy.ts';

interface GitFileRow {
  readonly path: string;
  readonly from?: string | null;
  readonly status: string;
  readonly add: number;
  readonly del: number;
  readonly binary: boolean;
  readonly rows: readonly ChangeRow[];
  readonly more?: number;
  readonly generated?: boolean;
  readonly staged?: boolean | null;
  readonly unstaged?: boolean | null;
  readonly hunkStage?: readonly { readonly staged: boolean; readonly unstaged: boolean }[] | null;
  /** a .env file's values sent as KEY=•••• */
  readonly masked?: boolean | null;
}
interface GitRead {
  readonly base: string;
  readonly body: { readonly kind: 'inline'; readonly files: readonly GitFileRow[] } | { readonly kind: 'blob' };
}

export const read = defineCommand<{ key: EntityKey; path: string; base: string; reveal: readonly string[] }, GitRead | null>({
  id: 'git.changes',
  owner: (a) => a.key,
  wire: (a) => ({ path: a.path, base: a.base, stageState: true, ...(a.reveal.length ? { reveal: [...a.reveal] } : {}) }),
  optimistic: 'none',
  pending: 'the Changes pane',
});
/** The same read on a phone (UX 2026-11-18 phone #14): its refusal is the pane's empty state, in words, not a toast
 *  holding a raw worktree path over it */
export const readHere = defineCommand({ ...read, quiet: true });
/** Reveal a .env file's captured edits in one turn (ADR-0050): the turn rows carry them masked. */
const unmasked = defineCommand<{ key: EntityKey; turnId: string; file: string }, { readonly edits?: readonly { readonly before: string; readonly after: string }[] }>({
  id: 'runtime.unmasked',
  owner: (a) => a.key,
  wire: (a) => ({ id: a.key.slice(a.key.indexOf(':') + 1), turnId: a.turnId, file: a.file }),
  optimistic: 'none',
  pending: 'the Reveal button, pressed at once; the masked edits stay until the values arrive',
});
const stage = defineCommand<{ key: EntityKey; dir: string; base: string; path: string; hunk?: number; hunkKey?: string; unstage: boolean }, { ok: boolean }>({
  id: 'git.stage',
  owner: (a) => a.key,
  wire: (a) => ({ dir: a.dir, base: a.base, path: a.path, unstage: a.unstage, ...(a.hunk !== undefined && { hunk: a.hunk, key: a.hunkKey }) }),
  optimistic: 'none',
  pending: 'the Stage toggle (flipped at once)',
});
const revert = defineCommand<
  { key: EntityKey; dir: string; base: string; path: string; hunk?: number; hunkKey?: string; restore?: string | null; hash?: string | null },
  { ok: boolean; hash?: string | null }
>({
  id: 'git.revert',
  owner: (a) => a.key,
  wire: (a) => ({
    dir: a.dir,
    base: a.base,
    path: a.path,
    ...(a.hunk !== undefined && { hunk: a.hunk, key: a.hunkKey }),
    // Undo: the text from before (null: the file wasn't there), if the file is still as the revert left it
    ...(a.restore !== undefined && { restore: a.restore, ...(a.hash != null && { hash: a.hash }) }),
  }),
  optimistic: 'none',
  pending: 'the hunk or file (hidden at once)',
});
/** The file's text now, for Undo (read before a revert, so Undo is exact). */
const source = defineCommand<{ key: EntityKey; id: string; path: string }, { text: string; hash: string }>({
  id: 'worktrees.source',
  owner: (a) => a.key,
  wire: (a) => ({ id: a.id, path: a.path }),
  optimistic: 'none',
  pending: 'none: read before a revert, which hides the hunk or file at once',
  quiet: true,
});
const commit = defineCommand<{ key: EntityKey; dir: string; message: string; all: boolean }, { ok: boolean; sha: string }>({
  id: 'git.commit',
  owner: (a) => a.key,
  wire: (a) => ({ dir: a.dir, message: a.message, all: a.all }),
  optimistic: 'none',
  pending: 'the committed files (gone at once)',
});

type Edits = readonly { readonly before: string; readonly after: string }[];
const shownKey = (turn: string, file: string): string => `${turn}\n${file}`;
type Wt = { baseRef?: string; base?: string; path?: string; cwd?: string };
type Raw = { cwd?: string; title?: string; summary?: { worktree?: Wt | null }; body?: { cwd?: string; ext?: Record<string, Wt | undefined> } };

const stageOf = (f: GitFileRow): StageState => (!f.staged && !f.unstaged ? 'none' : f.staged && !f.unstaged ? 'staged' : f.staged ? 'part' : 'stage');

const EMPTY_MARKS: ReadonlyMap<string, number> = new Map();

/** The edits wisp saw (no git), as the list's rows: each hunk's lines, a plain separator between hunks. A .env file's values read
 *  KEY=•••• under the Reveal line until it's revealed (v1 maskChange; git masks its own rows). */
function seenFiles(
  turns: readonly ChangeTurn[],
  id: string,
  cwd: string,
  maskOn: boolean,
  revealed: ReadonlySet<string>,
  shown: ReadonlyMap<string, Edits>,
  include?: (path: string) => boolean,
): ChangeFile[] {
  return filesOf(turns, id, cwd, (p) => seenMasked(p, maskOn, revealed), (turn, file) => shown.get(shownKey(turn, file)), include).map((f) => ({
    path: f.path,
    status: f.status === 'added' ? 'A' : f.status === 'deleted' ? 'D' : 'M',
    add: f.additions,
    del: f.deletions,
    binary: f.binary,
    revertable: false,
    ...(maskOn && isEnvFile(f.path) ? { env: revealed.has(f.path) ? ('shown' as const) : ('hidden' as const) } : {}),
    // between two edits, v1's plain separator (changes.js `{op: 'sep'}`): a snippet's surroundings aren't counted
    rows: f.hunks.flatMap((h, i) => [...(i ? [{ op: 'sep' }] : []), ...h.lines.map((l) => ({ op: l.kind === 'add' ? '+' : l.kind === 'del' ? '-' : ' ', text: l.text }))]),
  }));
}

export function ChangesPane(): JSX.Element {
  const shell = useShell();
  const task = useTask();
  const t = shell.t;
  // comments on lines (v1 comments.js): the card under a line, the ones waiting for the next follow-up
  const comments = commentStore(task.client);
  const all = useSignal(comments.value);
  // every comment waiting (an answer's too): the numbers are the tray's; the lines' are those with a file
  const list = createMemo(() => waiting(all(), task.key));
  const mine = createMemo(() => list().flatMap((c, k) => (isLine(c) ? [{ ...c, n: k + 1 }] : [])));
  // pre-indexed by file path so ChangesList looks up marks in O(1) per file instead of scanning per line
  const marksByPath = createMemo(() => {
    const out = new Map<string, Map<string, number>>();
    for (const c of mine()) {
      let m = out.get(c.file);
      if (!m) {
        m = new Map();
        out.set(c.file, m);
      }
      m.set(lineKey(c.side, c.line), c.n);
    }
    return out;
  });
  const [draft, setDraft] = createSignal<{ path: string; key: string; at: NonNullable<ReturnType<typeof placeOf>>; note: string } | undefined>();
  const raw = (): Raw | undefined => task.task() as Raw | undefined;
  // the task's worktree: wisp.worktrees' own ext, or the list row's summary (a task from v1's data has it there)
  const wt = (): Wt | undefined => raw()?.body?.ext?.['wisp.worktrees'] ?? raw()?.summary?.worktree ?? undefined;
  // git reads the worktree only (v1 GET /sessions/:id/changes; audit 2026-11-19: in place it diffed the shared checkout,
  // 198 files of every session's); memoized as primitives so streaming patches on the task row never re-run `files`
  const dir = createMemo((): string => worktreeDir(wt()));
  const folder = createMemo((): string => String(wt()?.path ?? wt()?.cwd ?? raw()?.body?.cwd ?? raw()?.cwd ?? ''));
  const base = createMemo((): string => wt()?.baseRef || wt()?.base || 'HEAD');
  // .env files shown as they are (v1 Reveal): the button flips at once, git's unmasked read follows
  const [revealed, setRevealedHere] = createSignal(revealedOf(task.id));
  // why the last read was refused: said where the files go, in words (a phone since UX 2026-11-18 phone #14, the browser
  // since the audit of 2026-11-19: a worktree cleaned up after the task toasted "no such folder: /Users/…")
  const [refused, setRefused] = createSignal<{ readonly code: string; readonly message: string } | undefined>();
  // what the read takes, compared by value: every patch of the task's row (a turn streaming, the full row after the
  // list's) made a new object here, so git was read again and the pane flipped to Loading and back (a Windows flake of
  // phone-fewer's gone-folder test; nothing jumps, AGENTS.md)
  const readArgs = createMemo(() => (dir() ? { path: dir(), base: base(), reveal: [...revealed()] } : undefined), undefined, {
    equals: (a, b) => a === b || (!!a && !!b && a.path === b.path && a.base === b.base && a.reveal.join('\n') === b.reveal.join('\n')),
  });
  const [git, { refetch }] = createResource(
    readArgs,
    (a) =>
      task.client
        .run(readHere, { key: task.key, ...a })
        .then((r) => (setRefused(undefined), r))
        .catch((e: unknown) => {
          setRefused(e instanceof Error ? { code: String((e as { code?: unknown }).code ?? ''), message: e.message } : { code: '', message: String(e) });
          return null;
        }),
  );
  // every turn whole while the pane is open: the task's topic sends older turns as stubs, without their edits (v1 #788)
  const turns = useSignal(task.client.entities.list<ChangeTurn>('turn'));
  // filtered to this task with shallow equality so turns updating in other tasks never trigger diff work here
  const taskTurns = createMemo(
    () => (turns() as readonly ChangeTurn[]).filter((x) => ofTask(x.session_id, task.id)),
    undefined,
    { equals: (a, b) => a.length === b.length && a.every((x, i) => x === b[i]) },
  );
  // git read again in place when the task settles (a running turn ends) or its uncommitted/ahead counts move; not on
  // every row patch (a turn streaming), nor after a refused read (a gone folder doesn't come back: Retry is there)
  const moved = createMemo(() => {
    const r = raw() as { state?: string; summary?: { toShip?: { dirty?: number; ahead?: number } | null } } | undefined;
    return { running: r?.state === 'running', counts: `${r?.summary?.toShip?.dirty ?? 0}|${r?.summary?.toShip?.ahead ?? 0}` };
  }, undefined, { equals: (a, b) => a.running === b.running && a.counts === b.counts });
  createEffect(
    on(
      moved,
      (now, before) => {
        const settled = before?.running === true && !now.running;
        if ((settled || before?.counts !== now.counts) && readArgs() && !refused()) void refetch();
      },
      { defer: true },
    ),
  );
  const whole = task.client.scope.child(`changes:${task.key}`);
  onCleanup(() => whole.dispose());
  keepTurnsWhole({
    scope: whole,
    bare: task.id,
    watch: (topic, sc) => task.client.watch(ownerOf(task.key), topic, sc),
    stubbed: () => taskTurns().some((x) => isStub(x.body)),
  });
  // Settings → Privacy's Hide .env values (git masks its own rows by it; the edits wisp saw follow it here)
  const maskSetting = useSignal(task.client.entities.row<SettingRow>('setting', entityKey(ownerOf(task.key), 'git.maskEnvFiles')));
  const maskOn = createMemo((): boolean => !(maskSetting()?.value === false || maskSetting()?.value === 'off'));
  // what you did before git answers again: a toggle flipped, a hunk or file reverted, files committed
  const [flipped, setFlipped] = createSignal(new Map<string, StageState>());
  const [going, setGoing] = createSignal(new Set<string>());
  const [open, setOpen] = createSignal<Set<string> | undefined>(undefined);
  const [viewed, setViewed] = createSignal(new Set<string>());
  const staging = createMemo((): boolean => !!wt()?.path || !!wt()?.cwd);
  // Hide: the line says hidden at once; the file's last read was unmasked (no `masked`) until git's masked one lands
  const [hiding, setHiding] = createSignal(new Set<string>());
  createEffect(on(git, () => setHiding(new Set<string>()), { defer: true }));
  const envOf = (f: GitFileRow): 'hidden' | 'shown' | undefined => envState(f, revealed()) ?? (hiding().has(f.path) ? 'hidden' : undefined);
  // what a revealed .env file's edits read unmasked (wisp saw them, no git): read by `files` as it's made
  const [seenShown, setSeenShown] = createSignal<ReadonlyMap<string, Edits>>(new Map());
  const files = createMemo((): ChangeFile[] => {
    const g = git();
    const myTurns = taskTurns();
    const cwd = folder();
    if (!g || g.body.kind !== 'inline') return seenFiles(myTurns, task.id, cwd, maskOn(), revealed(), seenShown());
    const st = staging();
    const fl = flipped();
    const go = going();
    const fromGit: ChangeFile[] = g.body.files
      .filter((f) => !go.has(f.path))
      .map((f) => ({
        path: f.path,
        status: f.status,
        add: f.add,
        del: f.del,
        binary: f.binary,
        rows: f.rows,
        ...(f.more ? { more: f.more } : {}),
        ...(f.generated ? { generated: true } : {}),
        ...(f.from ? { from: f.from } : {}),
        revertable: (f.status === 'M' || f.status === 'R') && !f.binary && f.rows.length > 0,
        ...(envOf(f) ? { env: envOf(f) as 'hidden' | 'shown' } : {}),
        ...(st ? { stage: fl.get(f.path) ?? stageOf(f) } : {}),
        ...(st && f.hunkStage ? { hunkStage: f.hunkStage.map((h) => (h.unstaged ? ('stage' as const) : h.staged ? ('unstage' as const) : undefined)) } : {}),
      }));
    // what wisp saw edited outside the folder git reads (an agent's own worktree): skip line diffing unless any exist
    if (!hasOutsideFolder(myTurns, task.id, cwd)) return fromGit;
    return [...fromGit, ...seenFiles(myTurns, task.id, cwd, maskOn(), revealed(), seenShown(), isOutsidePath)];
  });
  // the side panel's Changes tab counts what this read found (UX 2026-11-18 #5)
  createEffect(() => !git.loading && setChangeStat(task.key, { files: files().length }));
  // v1 opensNow (design/side-panel.md): all eligible files when ≤3, else only the first eligible (skipping generated, >300-row, viewed)
  const openSet = createMemo(() => open() ?? defaultOpen(files(), viewed()));
  const isOpen = (p: string): boolean => openSet().has(p);
  const touch = (): Set<string> => new Set(openSet());
  const toggle = (p: string): void => {
    const s = touch();
    if (s.has(p)) s.delete(p);
    else s.add(p);
    setOpen(s);
  };
  const expandable = createMemo(() => files().filter((f) => autoOpenable(f, viewed().has(f.path))));
  const allOpen = (): boolean => {
    const e = expandable();
    return e.length > 0 ? e.every((f) => isOpen(f.path)) : files().some((f) => isOpen(f.path));
  };
  const failed = (e: unknown): void => {
    shell.toast.show(e instanceof Error ? e.message : String(e), { tone: 'error' });
  };
  const after = (undo: () => void) => ({ ok: () => void refetch(), no: (e: unknown) => (undo(), failed(e)) });
  const onStage = (path: string, unstage: boolean): void => {
    const was = flipped();
    setFlipped(new Map(was).set(path, unstage ? 'stage' : 'staged'));
    const a = after(() => setFlipped(was));
    task.client.run(stage, { key: task.key, dir: dir(), base: base(), path, unstage }).then(() => (setFlipped(new Map()), a.ok()), a.no);
  };
  const onHunkStage = (path: string, hunk: number, hunkKey: string, unstage: boolean): void => {
    const a = after(() => undefined);
    task.client.run(stage, { key: task.key, dir: dir(), base: base(), path, hunk, hunkKey, unstage }).then(a.ok, a.no);
  };
  // no git: the edits wisp saw come masked in the turn rows; Reveal asks for each turn's change to that file
  const revealSeen = (path: string, show: boolean): void => {
    const g = git();
    if (g && g.body.kind === 'inline') return;
    const mine = (turns() as readonly ChangeTurn[]).filter((x) => ofTask(x.session_id, task.id) && x.id !== undefined);
    for (const turn of mine) {
      for (const c of changesOf(turn).filter((x) => relative(x.file, folder()) === path)) {
        const k = shownKey(turn.id ?? '', c.file);
        if (!show) {
          const m = new Map(seenShown());
          m.delete(k);
          setSeenShown(m);
          continue;
        }
        task.client.run(unmasked, { key: task.key, turnId: turn.id ?? '', file: c.file }).then(
          (r) => revealed().has(path) && r.edits && setSeenShown(new Map(seenShown()).set(k, r.edits)),
          (e: unknown) => (setRevealedHere(setRevealed(task.id, path, false)), failed(e)),
        );
      }
    }
  };
  const onReveal = (path: string, show: boolean): void => {
    if (!show) setHiding(new Set(hiding()).add(path));
    setRevealedHere(setRevealed(task.id, path, show));
    revealSeen(path, show);
  };
  const [goingHunks, setGoingHunks] = createSignal(new Set<string>());
  const onRevertHunk = (path: string, hunk: number, hunkKey: string): void => {
    const k = `${path}#${hunk}`;
    setGoingHunks(new Set(goingHunks()).add(k));
    const back = (): void => {
      const s = new Set(goingHunks());
      s.delete(k);
      setGoingHunks(s);
    };
    void textNow(path).then((before) =>
      task.client.run(revert, { key: task.key, dir: dir(), base: base(), path, hunk, hunkKey }).then(
        (r) => (reverted(undoSteps(path, undefined, before, r.hash)), void Promise.resolve(refetch()).then(back)),
        (e: unknown) => (back(), failed(e)),
      ),
    );
  };
  // the file's text now, for Undo; undefined when it can't be read as text (binary, too big, no folder): then no Undo
  // (a deleted file's "nothing there" is its status, never a failed read: Undo would delete what came back)
  const textNow = (path: string): Promise<string | undefined> =>
    task.client.run(source, { key: task.key, id: task.id, path }).then(
      (r) => r.text,
      () => undefined,
    );
  // Reverted, with Undo putting each file back as it was (no steps: no Undo); then git's read again (v1 reverted)
  const undo = (steps: readonly UndoStep[]): void =>
    void steps
      .reduce<Promise<unknown>>((p, u) => p.then(() => task.client.run(revert, { key: task.key, dir: dir(), base: base(), path: u.path, restore: u.text, hash: u.hash ?? null })), Promise.resolve())
      .catch(failed)
      .finally(() => void refetch());
  const reverted = (steps: readonly UndoStep[] | null): void => {
    shell.toast.show(t('shell-task.changes.reverted'), steps ? { action: { label: t('shell-task.changes.undo'), onClick: () => undo(steps) } } : {});
  };
  // a whole file asks first, saying what goes (v1 revertFile's danger ask)
  const [asking, setAsking] = createSignal<{ path: string; status: string; from?: string } | undefined>();
  const askRevert = (path: string): void => {
    const f = files().find((x) => x.path === path);
    setAsking({ path, status: f?.status ?? 'M', ...(f?.from && { from: f.from }) });
  };
  const revertFile = (path: string, status: string, from?: string): void => {
    const was = going();
    setGoing(new Set(was).add(path));
    // listed again once git's read is in (a rename's other half, a file still differing)
    const shownAgain = (): void => {
      const s = new Set(going());
      s.delete(path);
      setGoing(s);
    };
    void (status === 'D' ? Promise.resolve(null) : textNow(path)).then((before) =>
      task.client.run(revert, { key: task.key, dir: dir(), base: base(), path }).then(
        (r) => (reverted(undoSteps(path, from, before, r.hash)), void Promise.resolve(refetch()).then(shownAgain)),
        (e: unknown) => (setGoing(was), failed(e)),
      ),
    );
  };
  const changed = (): number => files().filter((f) => f.stage && f.stage !== 'none').length;
  const staged = (): number => files().filter((f) => f.stage === 'staged' || f.stage === 'part').length;
  const onCommit = (message: string): void => {
    const was = flipped();
    const all = staged() === 0;
    // committed files still differ from the base ("vs origin/main"), so they stay listed, as a reload lists them: at
    // once they read committed (nothing left to stage, the Commit bar goes), then git's read says the rest (audit F
    // tasks #3: they were hidden, "No changes yet", until a reload)
    const done = new Map(was);
    for (const f of files().filter((x) => x.stage === 'staged' || x.stage === 'part' || (all && x.stage === 'stage'))) done.set(f.path, 'none');
    setFlipped(done);
    task.client.run(commit, { key: task.key, dir: dir(), message, all }).then(
      (r) => (shell.toast.show(t('shell-task.changes.committed', { sha: r.sha })), void Promise.resolve(refetch()).then(() => setFlipped(new Map()))),
      (e: unknown) => (setFlipped(was), failed(e)),
    );
  };
  const viewedOf = (): string | undefined => {
    const n = files().filter((f) => viewed().has(f.path)).length;
    return n ? t('shell-task.changes.viewedOf', { n, total: files().length }) : undefined;
  };
  // keyed by (path, key) so typing in CommentCard updates its value in place without remounting the card or lines
  const draftSlot = createMemo(() => {
    const d = draft();
    return d ? `${d.path}\n${d.key}` : undefined;
  });
  const commentOpen = createMemo(() => {
    if (!draftSlot()) return undefined;
    const d = untrack(draft);
    if (!d) return undefined;
    return {
      path: d.path,
      key: d.key,
      card: (
        <CommentCard
          n={(() => {
            const cur = draft();
            const had = mine().find((c) => cur && c.file === cur.path && lineKey(c.side, c.line) === cur.key);
            return had ? had.n : list().length + 1;
          })()}
          where={draft() ? lineLabel({ file: draft()?.path ?? '', line: draft()?.at.line ?? 0, side: draft()?.at.side ?? 'new' }) : ''}
          code={draft()?.at.code ?? ''}
          value={draft()?.note ?? ''}
          placeholder={t('shell-task.comment.placeholder')}
          onInput={(v) => {
            const cur = draft();
            if (cur) setDraft({ ...cur, note: v });
          }}
          onSave={() => {
            const cur = draft();
            if (!cur || !cur.note.trim()) return;
            comments.set((c) => added(c, task.key, { ...cur.at, note: cur.note.trim() }) as never);
            setDraft(undefined);
          }}
          onCancel={() => setDraft(undefined)}
          labels={{ save: t('shell-task.comment.save'), cancel: t('shell-task.comment.cancel'), hint: t('shell-task.comment.hint') }}
        />
      ),
    };
  });
  const commentProp = {
    label: (o: ChangeRow) => t(o.op === '-' ? 'shell-task.comment.onRemoved' : 'shell-task.comment.onLine', { n: (o.op === '-' ? o.a : o.b) ?? 0 }),
    onAdd: (path: string, o: ChangeRow) => {
      const at = placeOf(path, o);
      if (at) setDraft({ path, key: rowKey(o) ?? '', at, note: mine().find((c) => c.file === path && lineKey(c.side, c.line) === rowKey(o))?.note ?? '' });
    },
    marks: (path: string) => marksByPath().get(path) ?? EMPTY_MARKS,
    get open() {
      return commentOpen();
    },
  };
  return (
    <>
    <Show when={files().length > 0} fallback={<ChangesEmpty loading={git.loading} {...(refused() && { refused: refused() as { code: string; message: string }, onRetry: () => void refetch() })} />}>
      <ChangesList
        part={'side.changes panel.changes' as `${string}.${string}`}
        files={files()}
        going={goingHunks()}
        open={isOpen}
        viewed={(p) => viewed().has(p)}
        onToggle={toggle}
        onViewed={(p) => {
          const s = new Set(viewed());
          if (s.has(p)) s.delete(p);
          else s.add(p);
          setViewed(s);
          const o = touch();
          if (s.has(p)) o.delete(p);
          setOpen(o);
        }}
        allOpen={allOpen()}
        onAll={() => {
          if (allOpen()) return setOpen(new Set<string>());
          const e = expandable();
          setOpen(new Set((e.length > 0 ? e : files().filter((f) => !f.binary && f.rows.length > 0 && !viewed().has(f.path))).map((f) => f.path)));
        }}
        // Open file… lists the worktree's files: only on git's diff, as v1 (audit 2026-11-19: on a folder git can't list
        // the palette closed at once, and the next keys ran as shortcuts: E marked the task done)
        {...(git() && {
          onFind: () =>
          openFinder(
            task.client,
            task.key,
            files().map((f) => f.path),
            {
              placeholder: t('shell-task.files.placeholder'),
              title: t('shell-task.changes.openFile'),
              empty: t('shell-task.files.empty'),
              noGit: t('shell-task.files.noGit'),
              toast: (text) => shell.toast.show(text),
            },
          ),
        })}
        onEdit={(file) => shell.navigate({ t: 'openSide', side: { kind: 'changes', id: task.key as never, file } })}
        onStage={onStage}
        onHunkStage={onHunkStage}
        onRevertHunk={onRevertHunk}
        onReveal={onReveal}
        comment={commentProp}
        fileMenu={(path) => ({
          menu: {
            items: [
              { value: 'revert', label: t('shell-task.changes.revertFile'), icon: 'replay', danger: true },
              { value: 'copy', label: t('shell-task.changes.copyPath'), icon: 'copy' },
            ],
            onSelect: (v: string) => (v === 'revert' ? askRevert(path) : void shell.platform.copy(path)),
          },
        })}
        {...(staging() && changed() > 0
          ? {
              commit: {
                count: staged() ? t('shell-task.changes.staged', { n: staged() }) : t('shell-task.changes.commitsAll', { n: changed() }),
                countTitle: staged() ? t('shell-task.changes.stagedTitle', { n: staged(), total: changed() }) : t('shell-task.changes.allTitle', { n: changed() }),
                placeholder: raw()?.title || t('shell-task.changes.message'),
                onCommit,
              },
            }
          : {})}
        labels={{
          files: (n) => t(n === 1 ? 'shell-task.changes.file' : 'shell-task.changes.files', { n }),
          vs: git() ? t('shell-task.changes.vs', { base: git()?.base ?? base() }) : t('shell-task.changes.seen'),
          vsTitle: git() ? t('shell-task.changes.vsTitle', { base: git()?.base ?? base() }) : t('shell-task.changes.seenTitle'),
          ...(viewedOf() ? { viewedOf: viewedOf() as string } : {}),
          expandAll: t('shell-task.changes.expandAll'),
          collapseAll: t('shell-task.changes.collapseAll'),
          openFile: t('shell-task.changes.openFile'),
          openFileTitle: t('shell-task.changes.openFileTitle'),
          stage: t('shell-task.changes.stage'),
          part: t('shell-task.changes.part'),
          staged: t('shell-task.changes.stagedOne'),
          viewed: t('shell-task.changes.viewed'),
          viewedTitle: (on) => t(on ? 'shell-task.changes.viewedOn' : 'shell-task.changes.viewedOff'),
          edit: t('shell-task.changes.edit'),
          more: t('shell-task.changes.more'),
          line: (n) => t('shell-task.changes.line', { n }),
          revert: t('shell-task.changes.revert'),
          revertTitle: t('shell-task.changes.revertTitle'),
          stageHunk: t('shell-task.changes.stage'),
          unstageHunk: t('shell-task.changes.unstage'),
          unchanged: (n) => t(n === 1 ? 'shell-task.changes.unchangedOne' : 'shell-task.changes.unchanged', { n }),
          commit: t('shell-task.changes.commit'),
          commitPlaceholder: t('shell-task.changes.message'),
          envHidden: (name) => t('shell-task.changes.envHidden', { name }),
          envShown: (name) => t('shell-task.changes.envShown', { name }),
          reveal: t('shell-task.changes.reveal'),
          hide: t('shell-task.changes.hide'),
          showMore: (n, left) =>
            left > n ? t('shell-task.changes.showMoreOf', { n, left }) : t(n === 1 ? 'shell-task.changes.showMoreOne' : 'shell-task.changes.showMore', { n }),
          moreFiles: (n) => t(n === 1 ? 'shell-task.changes.moreFilesOne' : 'shell-task.changes.moreFiles', { n }),
        }}
      />
    </Show>
    {/* v1 revertFile: "Revert math.js to origin/main? Every change this task made to math.js goes. Undo puts it back." */}
    <Show when={asking()}>
      {(a) => {
        const words = revertAsk(a().path, a().status, baseName(git()?.base, wt()?.baseRef, wt()?.base), t);
        return (
          <Dialog
            open
            role="alertdialog"
            size="sm"
            title={words.title}
            description={words.body}
            onOpenChange={(o) => !o && setAsking(undefined)}
            part="side.changes.revertAsk"
            footer={
              <Inline gap="4" justify="end">
                <Button variant="ghost" onClick={() => setAsking(undefined)}>
                  {t('shell-task.changes.revertAsk.cancel')}
                </Button>
                <Button variant="danger" part="side.changes.revertAsk.ok" onClick={() => {
                    const { path, status, from } = a();
                    setAsking(undefined);
                    revertFile(path, status, from);
                  }}>
                  {t('shell-task.changes.revertFile')}
                </Button>
              </Inline>
            }
          />
        );
      }}
    </Show>
    </>
  );
}

/** Nothing changed (or not read yet): one line, as v1's .cx-msg. */
export function ChangesEmpty(props: { readonly loading: boolean; readonly refused?: { readonly code: string; readonly message: string }; readonly onRetry?: () => void }): JSX.Element {
  const shell = useShell();
  // a refusal stays said while a read is under way (Retry, the folder back): its Retry shows the pending state, and the
  // next answer replaces it in place, never a flash of Loading between them
  return (
    <Show when={props.refused} fallback={<EmptyState icon="file" title={shell.t(props.loading ? 'shell-task.side.changes.loading' : 'shell-task.side.changes.empty')} compact />}>
      {(r) => (
        // a phone (UX 2026-11-18 phone #14): what happened in words; a gone folder is not an error to retry
        <EmptyState
          icon="file"
          part="side.changes.refused"
          title={shell.t(r().code === 'not_found' ? 'shell-task.side.changes.gone' : 'shell-task.side.changes.failed')}
          compact
          {...(r().code !== 'not_found' &&
            props.onRetry && {
              action: (
                <Button size="sm" variant="ghost" loading={props.loading} onClick={() => props.onRetry?.()}>
                  {shell.t('shell-task.side.changes.retry')}
                </Button>
              ),
            })}
        >
          {r().code === 'not_found' ? shell.t('shell-task.side.changes.goneBody') : r().message}
        </EmptyState>
      )}
    </Show>
  );
}
