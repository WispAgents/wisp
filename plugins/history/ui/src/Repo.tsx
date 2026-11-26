// The Repo tab (ADR-0033): a project's committed agent history, from everyone who worked on it, read from .wisp/
// and git (`history.list`). Grouped by month, filtered by words, a path or an agent; a row opens its read-only
// report (`history.report`) with Resume (a new task that carries on from there, on today's code).
import { createEffect, createMemo, createResource, createSignal, For, onCleanup, Show, type JSX } from 'solid-js';
import { defineCommand, entityKey, homeSetting, type PeerId, type PluginClient } from '@wisp/client';
import { PROJECT_ORDER, projectList, useShell } from '@wisp/shell';
import {
  Button,
  EmptyState,
  Icon,
  IconButton,
  Inline,
  Input,
  ListRow,
  Markdown,
  PageShell,
  SelectSetting,
  SettingsGroup,
  Stack,
  Text,
  englishT,
  timeLabel,
  useSignal,
} from '@wisp/ui';
import { t } from './i18n.ts';
import { byMonth, repoLine, resumePrompt, type RepoTask } from './model.ts';
import './i18n/lazy.ts';

interface Project {
  readonly id: string;
  readonly name: string;
  readonly path: string;
  readonly peer?: string;
}
interface Report {
  readonly task: RepoTask;
  readonly prompt: string;
  readonly report: string;
}

const list = defineCommand<{ peer: PeerId; repo: string; grep?: string; path?: string }, RepoTask[]>({
  id: 'history.list',
  owner: (a) => a.peer,
  wire: ({ peer: _p, ...a }) => a,
  optimistic: 'none',
  pending: 'the repo history',
});
const report = defineCommand<{ peer: PeerId; repo: string; id: string }, Report>({
  id: 'history.report',
  owner: (a) => a.peer,
  wire: ({ peer: _p, ...a }) => a,
  optimistic: 'none',
  pending: 'the report',
});
const resume = defineCommand<{ peer: PeerId; prompt: string; title: string; cwd: string; agent?: string; forkOf: string }, { id: string }>({
  id: 'runtime.create',
  owner: (a) => a.peer,
  wire: (a) => ({ prompt: a.prompt, title: a.title, cwd: a.cwd, ...(a.agent && { agent: a.agent }), lineage: { forkOf: a.forkOf } }),
  optimistic: 'none',
  pending: 'Resume',
});

/** The filter box: `auth/` is a path, the rest are words. */
const filterOf = (q: string): { grep?: string; path?: string } => {
  const w = q.trim();
  if (!w) return {};
  return w.endsWith('/') || (w.includes('/') && !w.includes(' ')) ? { path: w } : { grep: w };
};

export default function Repo(props: { readonly client: PluginClient; readonly params: { project?: string } }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const nav = useSignal(shell.nav);
  const phoneBar = (): boolean => shell.platform.phone || nav().layout === 'stack';
  // the agent by its harness's name, never its id (QA r15 #3)
  const harnesses = useSignal(shell.harnesses());
  const harnessName = (id: string): string => harnesses().find((h) => h.id === id)?.name ?? id;
  const say = t as (k: string, v?: Readonly<Record<string, string | number>>) => string;
  // the projects as every screen reads them (wisp.runtime's `project` rows on `config`), with keys so multi-peer mobile
  // routes `history.*` commands to the computer that actually owns the repo
  const projectRows = useSignal(c.entities.list<unknown>('project'));
  const projectKeys = useSignal(c.entities.keys('project'));
  const order = useSignal(homeSetting(c, PROJECT_ORDER)); // this wisp's order, not another computer's (doc25c)
  const projects = createMemo((): Project[] =>
    projectList(projectRows(), order(), projectKeys()).filter((p) => !!p.path),
  );
  const [picked, setPicked] = createSignal(props.params.project ?? '');
  const project = createMemo(() => projects().find((p) => p.id === picked()) ?? projects()[0]);
  const peer = (): PeerId => ((project()?.peer || c.home) as PeerId);
  // the first project once they arrive, so the picker shows which one this is
  createEffect(() => {
    const first = projects()[0];
    if (!picked() && first) setPicked(first.id);
  });
  const [q, setQ] = createSignal('');
  const [asked, setAsked] = createSignal('');
  const scope = c.scope.child('repo');
  onCleanup(() => scope.dispose());
  let cancel: (() => void) | undefined;
  const typed = (v: string): void => {
    setQ(v);
    cancel?.();
    cancel = scope.timeout(250, () => setAsked(v));
  };
  const [rows, { refetch: refetchRows }] = createResource(
    () => (project() ? ([peer(), project()?.path ?? '', asked()] as const) : undefined),
    ([p, repo, words]) => c.run(list, { peer: p, repo, ...filterOf(words) }).catch((): RepoTask[] => []),
  );
  const [open, setOpen] = createSignal<string | undefined>(undefined);
  const [shown] = createResource(
    () => (open() && project() ? ([peer(), project()?.path ?? '', open() ?? ''] as const) : undefined),
    ([p, repo, id]) => c.run(report, { peer: p, repo, id }).catch(() => undefined),
  );
  const month = (at: number): string => new Date(at).toLocaleDateString(undefined, { month: 'long', year: 'numeric' });
  const goResume = (r: Report): void => {
    const p = project();
    if (!p) return;
    const target = peer();
    void c
      .run(resume, {
        peer: target,
        prompt: resumePrompt(r.task.title, r.prompt, r.report),
        title: say('history.report.resumed', { title: r.task.title }),
        cwd: p.path,
        ...(r.task.agent && { agent: r.task.agent }),
        forkOf: r.task.id,
      })
      .then(
        (out) => shell.navigate({ t: 'openTask', id: entityKey(target, out.id) as never }),
        () => undefined,
      );
  };
  const refresh = (): void => {
    void refetchRows();
  };
  const projectLabel = (p: Project): string => {
    const multi = new Set(projects().map((x) => x.peer).filter(Boolean)).size > 1;
    return multi && p.peer ? `${p.name} · ${c.peers.name(p.peer as PeerId)}` : p.name;
  };
  let bodyEl: HTMLDivElement | undefined;
  createEffect(() => {
    open();
    if (bodyEl) bodyEl.scrollTop = 0;
  });
  const back = () => {
    if (open()) return { label: t('history.report.back'), onBack: () => setOpen(undefined) };
    if (phoneBar()) return { label: t('history.repo.back'), onBack: () => shell.navigate({ t: 'back' }) };
    return undefined;
  };
  return (
    <PageShell
      part="history.repo"
      icon="branch"
      title={open() ? (shown()?.task.title ?? t('history.repo.title')) : t('history.repo.title')}
      subtitle={open() ? undefined : t('history.repo.lede')}
      back={back()}
      bodyRef={(el) => (bodyEl = el)}
      trailing={
        <Show when={!open()}>
          <IconButton icon="replay" variant="ghost" size="sm" label={t('history.repo.refresh')} onClick={refresh} part="history.repo.refresh" />
        </Show>
      }
    >
      <Show
        when={open()}
        fallback={
          <>
            <SettingsGroup part="history.repo.head">
              <SelectSetting
                part="history.repo.project"
                title={t('history.repo.project')}
                source={{ value: () => project()?.id ?? '', set: setPicked }}
                items={projects().map((p) => ({ value: p.id, label: projectLabel(p) }))}
              />
            </SettingsGroup>
            <Input type="search" icon="search" size="sm" value={q()} placeholder={t('history.repo.filter')} aria-label={t('history.repo.filter')} onInput={typed} part="history.repo.filter" />
            <Show
              when={!rows.loading}
              fallback={
                <Text size="sm" tone="muted">
                  {t('history.repo.loading')}
                </Text>
              }
            >
              <Show when={(rows() ?? []).length > 0} fallback={<EmptyState icon="branch" compact title={project() ? t('history.repo.none') : t('history.repo.pick')} part="history.repo.empty" />}>
                <Inline justify="between" align="center" part="history.repo.summary">
                  <Text size="xs" tone="faint" weight="medium">
                    {say('history.repo.count', { count: (rows() ?? []).length })}
                  </Text>
                </Inline>
                <For each={byMonth(rows() ?? [], month)}>
                  {(g) => (
                    <Stack gap="1" part="history.repo.month">
                      <Text size="xs" tone="faint" weight="medium">
                        {g.label}
                      </Text>
                      <For each={g.tasks}>
                        {(r) => (
                          <ListRow
                            part="history.repo.row"
                            title={r.title}
                            sub={repoLine(r, say, harnessName)}
                            lead={<Icon name="branch" size="sm" tone="faint" />}
                            time={
                              <Text size="xs" tone="faint">
                                {timeLabel(r.createdAt, Date.now(), englishT)}
                              </Text>
                            }
                            onOpen={() => setOpen(r.id)}
                          />
                        )}
                      </For>
                    </Stack>
                  )}
                </For>
              </Show>
            </Show>
          </>
        }
      >
        <Stack gap="4" part="history.report">
          <Show
            when={shown()}
            fallback={
              <Text size="sm" tone="muted">
                {t('history.repo.loading')}
              </Text>
            }
          >
            {(r) => (
              <>
                <Stack gap="1">
                  <Text weight="semibold" size="lg">
                    {r().task.title}
                  </Text>
                  <Text size="sm" tone="muted">
                    {repoLine(r().task, say, harnessName)}
                  </Text>
                </Stack>
                <Stack gap="1">
                  <Text size="xs" tone="faint" weight="medium">
                    {t('history.report.asked')}
                  </Text>
                  <Text size="sm">{r().prompt}</Text>
                </Stack>
                <Stack gap="1">
                  <Text size="xs" tone="faint" weight="medium">
                    {t('history.report.report')}
                  </Text>
                  <Show
                    when={r().report.trim()}
                    fallback={
                      <Text size="sm" tone="muted">
                        {t('history.report.none')}
                      </Text>
                    }
                  >
                    <Markdown source={r().report} />
                  </Show>
                </Stack>
                <Inline gap="2">
                  <Button size="sm" variant="primary" icon="rerun" title={t('history.report.resumeTitle')} onClick={() => goResume(r())}>
                    {t('history.report.resume')}
                  </Button>
                </Inline>
              </>
            )}
          </Show>
        </Stack>
      </Show>
    </PageShell>
  );
}
