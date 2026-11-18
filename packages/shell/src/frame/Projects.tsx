// Projects (v1 features/projects.js, design/v1-reference/projects): add a folder or a GitHub URL (cloned for you),
// your projects in your order (drag a row, or Alt+↑/↓, v1 #772) with how many tasks each has, rename, recolour and
// remove; and the git repos found on this computer, one click to add. Every write is a resource op on wisp.runtime's
// `project` kind, shown at once and rolled back with a toast if refused.

import { createMemo, createResource, createSignal, onMount, type JSX } from 'solid-js';
import { defineCommand, entityKey, type PeerId, type PluginClient } from '@wisp/client';
import { PathField, ProjectsSheet, reorderable, useSignal } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { usePluginClient } from '../slot.tsx';
import { SESSION, type SessionRow } from '../rows.ts';
import { useProjects } from './projects.ts';
import { projectOf, type ProjectView } from './rows-view.ts';
import { addedName, folderOf, nameFor, whereIs } from './project-names.ts';
import '../i18n/lazy.ts';

const COLORS = ['violet', 'teal', 'amber', 'rose', 'sky', 'lime', 'orange', 'indigo', 'slate'] as const;
const IS_URL = /^(https?:\/\/|git@|ssh:\/\/)/;

interface Found {
  readonly path: string;
  readonly name: string;
}

const row = (peer: string, name: string, spec: Record<string, unknown>) => ({ e: 'project', id: entityKey(peer as PeerId, name), op: 'upsert' as const, v: { id: name, body: spec } as never });

/** Add or change a project: its spec, as a resource. */
const putProject = defineCommand<{ peer: string; name: string; spec: Record<string, unknown> }>({
  id: 'resources.apply',
  owner: (a) => a.peer,
  wire: (a) => ({ resources: [{ kind: 'project', name: a.name, spec: a.spec }], trust: true }),
  optimistic: (_b, a) => [row(a.peer, a.name, a.spec)],
});
/** Add a folder: named on the server (runtime.projectAdd, ADR-0077), so a taken name gets `-2` there, never an
 * overwrite; its row shows at once under the name it will most likely get (`guess`), replaced by the server's. */
const addProject = defineCommand<{ peer: string; path: string; guess: string }, { name: string; path: string; created: boolean }>({
  id: 'runtime.projectAdd',
  owner: (a) => a.peer,
  wire: (a) => ({ path: a.path }),
  optimistic: (_b, a) => [row(a.peer, a.guess, { path: a.path })],
});
const dropProject = defineCommand<{ peer: string; name: string }>({
  id: 'resources.apply',
  owner: (a) => a.peer,
  wire: (a) => ({ resources: [], destroy: [`project.${a.name}`] }),
  optimistic: (_b, a) => [{ e: 'project', id: entityKey(a.peer as PeerId, a.name), op: 'del' }],
});
const renameProject = defineCommand<{ peer: string; from: string; to: string; spec: Record<string, unknown> }>({
  id: 'resources.rename',
  owner: (a) => a.peer,
  wire: (a) => ({ kind: 'project', from: a.from, to: a.to }),
  optimistic: (_b, a) => [{ e: 'project', id: entityKey(a.peer as PeerId, a.from), op: 'del' }, row(a.peer, a.to, a.spec)],
});
/** v1 GET /fs?q=: the folders the typed path could go on to (answered in the form typed: `~/…` stays `~/…`) */
const folders = defineCommand<{ peer: string; q: string }, string[]>({ id: 'runtime.folders', owner: (a) => a.peer, wire: (a) => ({ q: a.q }), optimistic: 'none', pending: 'the folder suggestions' });
const discover = defineCommand<{ peer: string }, Found[]>({ id: 'runtime.discover', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'Found on this machine' });
/** v1 GET /projects/github: your GitHub repos (gh's sign-in), newest first, minus the projects here */
interface GithubRepo {
  readonly repo: string;
  readonly url: string;
  readonly description: string;
  readonly private: boolean;
  readonly fork: boolean;
}
const githubRepos = defineCommand<{ peer: string; paths: string[] }, GithubRepo[]>({
  id: 'git.githubRepos',
  owner: (a) => a.peer,
  wire: (a) => ({ paths: a.paths }),
  optimistic: 'none',
  pending: 'Your GitHub repos',
});
const clone = defineCommand<{ peer: string; url: string }, { dir: string }>({ id: 'git.clone', owner: (a) => a.peer, wire: (a) => ({ url: a.url }), optimistic: 'none', pending: 'Add' });

export default function Projects(_p: { readonly client: PluginClient; readonly params: unknown }): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const home = usePluginClient('shell').home;
  const { list, reorder } = useProjects(shell, home);
  const rows = useSignal(shell.client.entities.list<unknown>('project'));
  const sessions = useSignal(shell.client.entities.list<SessionRow>(SESSION));
  const [source, setSource] = createSignal('');
  const [adding, setAdding] = createSignal(false);
  const [found, { refetch }] = createResource(() => shell.client.commands.run(discover, { peer: home }).catch((): Found[] => []));
  const specOf = (id: string): Record<string, unknown> => {
    const r = rows().find((x) => (x as { id?: string }).id === id) as { body?: Record<string, unknown> } | undefined;
    return { ...(r?.body ?? {}) };
  };
  const counts = createMemo(() => {
    const out = new Map<string, number>();
    for (const s of sessions()) {
      const p = projectOf(String(s.summary['cwd'] ?? ''), list());
      if (p) out.set(p.id, (out.get(p.id) ?? 0) + 1);
    }
    return out;
  });
  const add = (path: string): void => {
    // a folder that's a project already stays one project (v1 addProject returned the one it had)
    const had = list().find((p) => folderOf(p.path) === folderOf(path));
    if (had) return void shell.toast.show(t('shell.projects.already', { name: had.name }));
    const guess = addedName(
      path,
      list().map((p) => p.id),
    );
    void shell.client.commands.run(addProject, { peer: home, path, guess }).then(
      (r) => {
        // another spelling of a folder that's a project already (`/repo/.`, `~/repo`): the server kept that one
        if (!r.created) shell.toast.show(t('shell.projects.already', { name: r.name }));
        void refetch();
      },
      () => undefined,
    );
  };
  // your GitHub repos (v1 #ghrepos), asked once the projects are known; a clone in flight leaves its row
  const [repos] = createResource(
    () => list().map((p) => p.path),
    (paths) => shell.client.commands.run(githubRepos, { peer: home, paths }).catch((): GithubRepo[] => []),
  );
  const [cloning, setCloning] = createSignal<ReadonlySet<string>>(new Set());
  const [ghAll, setGhAll] = createSignal(false);
  const ghRepos = (): GithubRepo[] => (repos() ?? []).filter((r) => !cloning().has(r.url));
  const cloneRepo = (url: string): void => {
    setCloning(new Set([...cloning(), url]));
    void shell.client.commands
      .run(clone, { peer: home, url })
      .then((c) => add(c.dir), () => setCloning(new Set([...cloning()].filter((u) => u !== url))));
  };
  const submit = (): void => {
    const v = source().trim();
    if (!v) return;
    setSource('');
    if (!IS_URL.test(v)) return add(v);
    setAdding(true);
    void shell.client.commands
      .run(clone, { peer: home, url: v })
      .then(
        // named after its clone's folder (the repo's name), `-2` on the server when that's taken
        (c) => add(c.dir),
        () => setSource(v),
      )
      .finally(() => setAdding(false));
  };
  const ids = (): string[] => list().map((p) => p.id);
  const recolor = (p: ProjectView, color: string): void => void shell.client.commands.run(putProject, { peer: home, name: p.id, spec: { ...specOf(p.id), color } }).catch(() => undefined);
  // rename in place: the row's name becomes a field (Enter keeps it, Escape leaves it)
  const [editing, setEditing] = createSignal<string | undefined>(undefined);
  const [draft, setDraft] = createSignal('');
  const rename = (p: ProjectView): void => {
    setDraft(p.name);
    setEditing(p.id);
  };
  const saveName = (p: ProjectView): void => {
    setEditing(undefined);
    const to = nameFor(
      draft(),
      list()
        .map((x) => x.id)
        .filter((x) => x !== p.id),
    );
    if (!draft().trim() || to === p.id) return;
    void shell.client.commands.run(renameProject, { peer: home, from: p.id, to, spec: specOf(p.id) }).catch(() => undefined);
  };
  // the row's name, edited in place (v1 .pname): Enter or leaving it keeps it, Escape puts it back
  const renameTo = (id: string, name: string): void => {
    const p = list().find((x) => x.id === id);
    if (!p) return;
    setDraft(name);
    saveName(p);
  };
  // v1 tilde(): your home as `~`
  const tilde = (path: string): string => path.replace(/^\/(?:Users|home)\/[^/]+/, '~').replace(/^[A-Za-z]:\\Users\\[^\\]+/, '~');
  // v1's hint: `wisp add` set as code
  const hint = (): (string | { code: string })[] => {
    const [before, after] = t('shell.projects.hint').split('wisp add');
    return after === undefined ? [before ?? ''] : [before ?? '', { code: 'wisp add' }, after];
  };
  // the caret in the path field as it opens (audit A F26: the dialog focused its first tab, so typing went nowhere)
  let pathEl: HTMLInputElement | undefined;
  onMount(() => requestAnimationFrame(() => pathEl?.focus()));
  // the empty state points only at what's there: "pick one below" with nothing below read as a broken page (F26)
  const below = (): boolean => (found() ?? []).length > 0 || ghRepos().length > 0;
  const openMachines = (): void => shell.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane: 'machines' } } });
  return (
    <ProjectsSheet
      places={[{ id: '', label: t('shell.card.thisComputer'), icon: 'folder', on: true }]}
      onPlace={() => undefined}
      addMachine={{ label: t('shell.projects.machine'), title: t('shell.projects.machineTitle'), onClick: openMachines }}
      field={
        <PathField
          ref={(el) => (pathEl = el)}
          size="lg"
          value={source()}
          onInput={setSource}
          suggest={(q) => shell.client.commands.run(folders, { peer: home, q })}
          skip={(q) => IS_URL.test(q)}
          onPick={(p) => {
            setSource('');
            add(p);
          }}
          onEnter={submit}
          placeholder={t('shell.projects.placeholder')}
          aria-label={t('shell.projects.add')}
        />
      }
      browse={shell.platform.pickFolder ? { label: t('shell.projects.browse'), onClick: () => void shell.platform.pickFolder?.().then((dir) => dir && add(dir)) } : undefined}
      add={{ label: IS_URL.test(source().trim()) ? t('shell.projects.clone') : t('shell.projects.add'), onClick: submit, busy: adding() }}
      hint={hint()}
      rows={list().map((p) => ({
        id: p.id,
        name: p.name,
        path: tilde(p.path),
        count: t((counts().get(p.id) ?? 0) === 1 ? 'shell.projects.task' : 'shell.projects.tasks', { count: counts().get(p.id) ?? 0 }),
        color: COLORS.includes(p.color as never) ? (p.color as string) : 'slate',
        reorder: reorderable('projects', p.id, ids, reorder),
      }))}
      empty={t(below() ? 'shell.projects.none' : 'shell.projects.noneAbove')}
      nameLabel={t('shell.projects.name')}
      colourLabel={t('shell.projects.colour')}
      colours={COLORS.map((c) => ({ value: c, label: t(`shell.projects.color.${c}`) }))}
      onColour={(id, c) => {
        const p = list().find((x) => x.id === id);
        if (p) recolor(p, c);
      }}
      onRename={renameTo}
      removeLabel={(name) => t('shell.projects.remove', { name })}
      onRemove={(id) => void shell.client.commands.run(dropProject, { peer: home, name: id }).catch(() => undefined)}
      github={{
        heading: t('shell.projects.github'),
        sub: t('shell.projects.githubHint'),
        items: (ghAll() ? ghRepos() : ghRepos().slice(0, 6)).map((r) => {
          const [owner = '', name = r.repo] = r.repo.split('/');
          const marks = [r.private && t('shell.projects.private'), r.fork && t('shell.projects.fork')].filter(Boolean).join(' · ');
          return { url: r.url, name, owner: marks ? `${owner} · ${marks}` : owner, title: r.description || r.repo };
        }),
        onClone: cloneRepo,
        more: ghRepos().length > 6 ? { label: ghAll() ? t('shell.projects.showFewer') : t('shell.projects.showAll', { count: ghRepos().length }), onClick: () => setGhAll(!ghAll()) } : undefined,
      }}
      found={{
        heading: t('shell.projects.found'),
        sub: t('shell.projects.foundHint'),
        items: (found() ?? []).map((f) => ({ path: f.path, name: f.name, where: whereIs(f.path), label: t('shell.projects.addFound', { name: f.name }) })),
        onAdd: (path) => add(path),
      }}
    />
  );
}
