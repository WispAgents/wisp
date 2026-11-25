// What an answer's blocks may ask of the open task (BlockContext, packages/ui/src/blocks/types.ts): send a reply,
// put text in the reply box, run a command in the task's folder (and show what it did), open another task or a
// link, allow or deny a blocked turn, read another task's title and state. Live only on the latest answer.
import { createSignal, getOwner, onCleanup } from 'solid-js';
import type { EntityKey } from '@wisp/client';
import { entityKey, ownerOf, untracked } from '@wisp/client';
import { useSignal, type BlockContext, type CapturedChange, type DocText, type EnvReveal, type StuffRef, type TaskRef } from '@wisp/ui';
import type { ShellContext } from '../contract.ts';
import { usePluginClient } from '../slot.tsx';
import { SESSION, type SessionRow } from '../rows.ts';
import { appState, approveBlocked, docSource, exec, reveal } from './commands.ts';
import { docRelativePath, isLink, mediaUrl, sessionFilePath } from './answer-of.ts';
import { peerMedia } from './media-src.ts';
import { keepHost } from './keep.ts';
import { fileUrl } from '../editor.ts';
import '../i18n/lazy.ts';

export interface AnswerHost {
  readonly shell: ShellContext;
  readonly key: EntityKey;
  readonly turnId: string;
  /** what the turn's refusals allow (v1 #777): `allowable` (the server kept them: Allow & retry), `unallowed` (a Full
   *  auto run its harness still refused: a note, no retry), `none` */
  readonly blocked?: () => 'allowable' | 'unallowed' | 'none';
  /** this answer is the task's latest (read live: a new turn makes the older answers inert) */
  readonly latest: () => boolean;
  /** the task's title (a hero repeating it says nothing) */
  readonly title?: () => string;
  /** the transcript shows: the hero says its state and meta again (v1: not the trail then) */
  readonly transcript?: () => boolean;
  readonly meta?: () => readonly string[];
  readonly reply: (text: string) => void;
  readonly draft: (text: string) => void;
  readonly openChanges: () => void;
  /** the turn's captured changes: `diff` blocks and `files` rows that name a file show its edits */
  readonly changes?: () => readonly CapturedChange[];
  /** which masked .env views show (env-reveal.ts), built once per turn; without it a block keeps its own Reveal */
  readonly env?: EnvReveal;
  /** what the turn's `app` blocks saved (`ext["wisp.apps"]`) */
  readonly apps: () => Readonly<Record<string, unknown>>;
  /** wisp's own tour (v1 `s.agent === 'wisp'`): no Save to My Stuff on it */
  readonly tour?: boolean;
}

export function answerContext(h: AnswerHost): BlockContext {
  const { shell, key } = h;
  const run = shell.client.commands.run;
  const peer = ownerOf(key);
  // called while a view sets up (TaskView, Compare): the shell's own client says which wisp is this one
  const shellClient = usePluginClient('shell');
  const home = shellClient.home;
  const keyOf = (id: string): EntityKey => (id.includes(':') ? (id as EntityKey) : entityKey(peer, id));
  // where a `preview` block's page loads from here (ADR-0064: a box's localhost at the grid's preview address); set
  // up once per answer, read reactively by the block; the first plugin that answers wins. Read once on purpose
  // (untracked): `ctx={answerContext(…)}` is a JSX getter, run again on each read of the prop, so a watcher here would
  // be made per read (round 324 review)
  const previews = untracked(shell.previewAddresses?.() ?? (() => [])).map((d) => d.map({ client: d.client, task: key }));
  return {
    // a task's answers show as they are: no rise, no count-up, every time the task opens (owner, 2026-11-21)
    reveal: false,
    // a .env file's values in a code or diff block: masked, revealed per task for the tab's life (env-reveal.ts)
    ...(h.env && { env: h.env }),
    ...(h.changes && { changes: h.changes }),
    get latest() {
      return h.latest();
    },
    get inTask() {
      return !h.transcript?.();
    },
    get heroMeta() {
      return h.transcript?.() ? (h.meta?.() ?? []) : [];
    },
    get taskTitle() {
      return h.title?.() ?? '';
    },
    reply: h.reply,
    draft: h.draft,
    run: async (command) => {
      const at = Date.now();
      const r = await run(exec, { id: key, command });
      return { exitCode: r.code, output: r.output, durationMs: Date.now() - at };
    },
    get allow() {
      return (h.blocked?.() ?? 'allowable') === 'allowable' ? () => run(approveBlocked, { id: key, turnId: h.turnId, allow: true }) : undefined;
    },
    get deny() {
      return (h.blocked?.() ?? 'allowable') === 'allowable' ? () => run(approveBlocked, { id: key, turnId: h.turnId, allow: false }) : undefined;
    },
    get unallowed() {
      return h.blocked?.() === 'unallowed';
    },
    openTask: (id) => shell.navigate({ t: 'openTask', id: keyOf(id) as never }),
    openUrl: (url) => shell.platform.openExternal(url),
    // a file the answer names: in the editor Settings names, at its line, on this computer's own tasks (v1
    // openInEditor); elsewhere, or with no editor, the Changes pane (audit A #10: it always opened an empty Changes pane)
    openFile: (path, line) => {
      const r = shell.client.entities.peek<{ body?: { cwd?: string; ext?: Record<string, { path?: string } | undefined> }; summary?: { cwd?: string } }>(SESSION, key);
      const dir = String(r?.body?.ext?.['wisp.worktrees']?.path ?? r?.body?.cwd ?? r?.summary?.cwd ?? '');
      const editor = String(shell.client.entities.peek<{ value?: unknown }>('setting', entityKey(peer, 'settings.editor'))?.value ?? '');
      const url = peer === home ? fileUrl(editor, dir, path, line) : undefined;
      if (url) shell.platform.openExternal(url);
      else h.openChanges();
    },
    // the page's own URL, or (the phone app, which no wisp serves) one the task's computer's transport gives
    media: (p) => {
      const rel = mediaUrl(home, key, p);
      if (!rel || isLink(p)) return rel;
      const got = peerMedia(shell.client, shellClient.http(peer), peer, sessionFilePath(key, p), Date.now());
      return got === 'relative' ? rel : got;
    },
    // Live read when a `doc` block has no snapshotted `text` yet (mid-turn Chat mode blocks or turns from before keep_docs).
    docText: (path) => {
      const r = shell.client.entities.peek<{ body?: { cwd?: string; ext?: Record<string, { path?: string; cwd?: string } | undefined> }; summary?: { cwd?: string } }>(SESSION, key);
      const dir = String(r?.body?.ext?.['wisp.worktrees']?.cwd ?? r?.body?.ext?.['wisp.worktrees']?.path ?? r?.body?.cwd ?? r?.summary?.cwd ?? '');
      const rel = docRelativePath(dir, path);
      if (!rel) return () => ({ kind: 'failed', reason: '' });
      const [state, setState] = createSignal<DocText>({ kind: 'loading' });
      let alive = true;
      if (getOwner()) onCleanup(() => { alive = false; });
      void run(docSource, { id: key, path: rel }).then(
        (res) => alive && setState({ kind: 'ready', text: res.text }),
        (e: unknown) => alive && setState({ kind: 'failed', reason: e instanceof Error ? e.message : String(e) }),
      );
      return state;
    },
    previewUrl: (url) => {
      for (const m of previews) {
        const to = m(url);
        if (to) return to;
      }
      return undefined;
    },
    // Show in folder: only on a computer (a phone app has no folder to open, v1 MOBILE)
    ...(!shell.platform.phone && {
      revealFile: (path: string) =>
        void run(reveal, { id: key, path }).then(
          (r) => !r.ok && shell.toast.error(shell.t('shell.reveal.failed', { path: r.path })),
          (e: unknown) => shell.toast.error(e instanceof Error ? e.message : String(e)),
        ),
    }),
    appState: {
      get: (block) => h.apps()[block],
      set: (block, state) => void run(appState, { id: key, turnId: h.turnId, block, state }).catch(() => undefined),
    },
    // My Stuff's page (wisp.stuff's view); its items are `stuff.item` rows on the `stuff` topic everyone watches
    openStuff: () => shell.navigate({ t: 'openView', id: 'stuff.page' as never, params: null }),
    // Save to My Stuff on its pictures, videos, docs and code (./keep.ts); the tour's answers keep nothing
    ...(!h.tour && { keep: keepHost(shell, home, key, h.turnId) }),
    stuff: (id) => {
      const row = useSignal(shell.client.entities.row<{ id: string; title?: string; name?: string; kind?: string; excerpt?: string }>('stuff.item', keyOf(id)));
      return (): StuffRef | undefined => {
        const it = row();
        return it ? { id: it.id, title: it.title || it.name || '', kind: it.kind ?? '', ...(it.excerpt && { sub: it.excerpt }) } : undefined;
      };
    },
    task: (id) => {
      // called while the task block sets up, so the row stays live in it
      const row = useSignal(shell.client.entities.row<SessionRow>(SESSION, keyOf(id)));
      return (): TaskRef | undefined => {
        const s = row();
        return s ? { id: s.id, title: s.title, state: s.archived ? 'archived' : s.state, ...(s.agent && { agent: s.agent }) } : undefined;
      };
    },
  };
}
