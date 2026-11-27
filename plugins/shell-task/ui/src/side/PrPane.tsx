// The side panel's PR pane (v1 features/panel.js paintPr): the task's PRs from `ship.details` (one chip each when
// there are several), the title as a link, #n · state · branch → base · author, the verdict with its one step
// (Merge, Fix conflicts), merge risk (PrRisk), checks with the passed folded, reviewers, the conversation (outdated threads and bots
// folded), the files, and Open on GitHub · Checked … · Check now. Skeleton rows while the first answer comes.
import { For, Show, createEffect, createMemo, createResource, createSignal, on, type JSX } from 'solid-js';
import { defineCommand, type EntityKey } from '@wisp/client';
import { Banner, Button, CheckList, CheckRow, Dialog, EmptyState, FileStatList, FileStatRow, FoldRow, IconButton, Inline, Link, PaneHead, Skeleton, Stack, Text, ToneChip, Wrap, timeLabel } from '@wisp/ui';
import type { Tone as KitTone } from '@wisp/tokens';
import { useShell } from '../shell.ts';
import { useTask } from '../task-context.tsx';
import { PrRisk } from './PrRisk.tsx';
import { checkGroups, dur, fixText, gist, hasPr, prFiles, talk, verdict, type CheckItem, type PrNote, type PrPanel, type Tone } from './pr.ts';
import '../i18n/lazy.ts';

const bare = (k: string): string => k.slice(k.indexOf(':') + 1);
type S = { key: EntityKey; session: string };
const wire = (a: S): { session: string } => ({ session: a.session });
const details = defineCommand<S, PrPanel[]>({ id: 'ship.details', owner: (a) => a.key, wire, optimistic: 'none', pending: 'the PR pane' });
const refresh = defineCommand<S, unknown>({ id: 'ship.refresh', owner: (a) => a.key, wire, optimistic: 'none', pending: 'Check now' });
const merge = defineCommand<S, { kept?: readonly string[] }>({ id: 'ship.merge', owner: (a) => a.key, wire, optimistic: 'none', pending: 'Merge' });
const fixConflicts = defineCommand<S, unknown>({ id: 'ship.fix_conflicts', owner: (a) => a.key, wire, optimistic: 'none', pending: 'Fix conflicts' });
// Ask to fix (audit E4): the end of a failing Actions job's log ("" for any other link)
const checkLog = defineCommand<S & { readonly url: string }, unknown>({
  id: 'ship.check_log',
  owner: (a) => a.key,
  wire: (a) => ({ session: a.session, url: a.url }),
  optimistic: 'none',
  pending: 'the Ask to fix button',
});

const TONE: Readonly<Record<Tone, KitTone>> = { merged: 'accent', closed: 'neutral', fail: 'bad', pending: 'warn', draft: 'neutral', pass: 'good' };
const STATE: Readonly<Record<string, string>> = { OPEN: 'shell-task.pr.open', MERGED: 'shell-task.pr.stateMerged', CLOSED: 'shell-task.pr.stateClosed' };

export function PrPane(): JSX.Element {
  const shell = useShell();
  const task = useTask();
  const t = shell.t;
  // the kit's own words (time.now, time.minutes…) live under `kit.` in the shell's catalogue: a bare key showed "time.now"
  const kitT = (key: string, vars?: Readonly<Record<string, string | number>>): string => t(`kit.${key}`, vars);
  const c = task.client;
  const args = (): S => ({ key: task.key, session: bare(task.key) });
  const [at, setAt] = createSignal(0);
  // the PRs wisp.ship keeps on the task's row (v1 prsOf): none, and there is nothing to ask GitHub about
  const known = (): boolean => hasPr(task.task());
  const [list, { refetch }] = createResource(
    () => (known() ? task.key : undefined),
    // a task ship doesn't track (no worktree, never shipped) has no PR: v1's "This task has no pull request."
    () =>
      c.run(details, args()).then(
        (l) => (setAt(Date.now()), l),
        () => [] as PrPanel[],
      ),
  );
  // Refresh PR details in place when the server updates the task's PR summary or state.
  const prSig = createMemo(() => {
    const r = task.task() as { state?: string; summary?: { pr?: unknown } } | undefined;
    return `${r?.state ?? ''}|${JSON.stringify(r?.summary?.pr ?? null)}`;
  });
  createEffect(
    on(
      prSig,
      () => {
        if (known()) void refetch();
      },
      { defer: true },
    ),
  );
  const [picked, setPicked] = createSignal(0);
  const [open, setOpen] = createSignal<ReadonlySet<string>>(new Set());
  const unfold = (k: string): void => void setOpen(new Set([...open(), k]));
  const fold = (k: string): void => void setOpen(new Set([...open()].filter((x) => x !== k)));
  const raw = (): { agent?: string; state?: string; status?: string; body?: { conflictFix?: { step?: string } } } => (task.task() ?? {}) as never;
  const pr = createMemo((): PrPanel | undefined => {
    const l = list() ?? [];
    return l[Math.min(picked(), l.length - 1)];
  });
  // Ask to fix (v1 panel.js askToFix): the check and its log's end in the reply box, not sent (you look first)
  const [asking, setAsking] = createSignal('');
  const askFix = (k: CheckItem, n: number): void => {
    setAsking(k.name);
    void c
      .run(checkLog, { ...args(), url: k.url ?? '' })
      .then(
        (l) => (typeof l === 'string' ? l : ''),
        () => '',
      )
      .then((log) => {
        setAsking('');
        // a phone's panel covers the reply box: it closes, so the text is in view (v1 toReply: `if (isStacked()) close()`)
        if (shell.nav().layout === 'stack') shell.navigate({ t: 'closeSide' });
        const box = shell.prompts?.get('reply');
        box?.set(fixText(k.name, n, k.url, log));
        box?.focus();
      });
  };
  const quiet = (p: Promise<unknown>): void =>
    void p.then(
      () => refetch(),
      () => {},
    );
  // Merge asks first (v1 ship.js mergePr → ask(); audit A tasks #5: the pane merged at a click). A worktree with work
  // the merge doesn't take stays, and the answer says so: that's the one line shown after.
  const [mergeAsk, setMergeAsk] = createSignal(false);
  const doMerge = (): void => {
    setMergeAsk(false);
    void c.run(merge, args()).then(
      (r) => {
        for (const line of r?.kept ?? []) shell.toast.show(line);
        void refetch();
      },
      () => {},
    );
  };
  const fixing = (): boolean => ['fixing', 'pushing'].includes(raw().body?.conflictFix?.step ?? '');
  const canAsk = (): boolean => raw().agent !== 'wisp' && raw().state !== 'running' && raw().status !== 'running';

  const who = (n: { author: string; bot?: boolean }): JSX.Element => (
    <Text size="sm" weight="medium" truncate>
      {n.author}
    </Text>
  );
  const note = (n: PrNote): JSX.Element => (
    <Stack gap="1" part="side.pr.note" as="li">
      <Inline gap="2" justify="between">
        {who(n)}
        <Show when={n.at}>
          {(a) => (
            <Text size="xs" tone="muted">
              {timeLabel(a(), Date.now(), kitT)}
            </Text>
          )}
        </Show>
      </Inline>
      <Text size="sm" tone="muted">
        {gist(n.body)}
      </Text>
    </Stack>
  );

  return (
    <Stack gap="6" part="side.pr" grow>
      <Show
        when={known() ? list() : []}
        fallback={
          <Stack gap="2" part="side.pr.skeleton" aria-hidden="true">
            <Skeleton width="2/3" />
            <Skeleton width="1/2" />
            <Skeleton width="full" />
            <Skeleton width="1/2" />
            <Skeleton width="1/2" />
          </Stack>
        }
      >
        <Show when={pr()} fallback={<EmptyState icon="merge" title={t('shell-task.pr.none')} compact />}>
          {(p) => {
            const v = createMemo(() => verdict(p(), { fixing: fixing(), canFix: canAsk() }));
            const g = createMemo(() => checkGroups(p().checkList ?? []));
            const said = createMemo(() => talk(p()));
            const files = createMemo(() => prFiles(p(), open().has('files')));
            return (
              <>
                <Show when={(list() ?? []).length > 1}>
                  <Wrap gap="2" part="side.pr.chips">
                    <For each={list()}>
                      {(x, k) => (
                        <ToneChip
                          label={x.repo || `#${x.number}`}
                          suffix={x.repo ? `#${x.number}` : undefined}
                          tone={TONE[verdict(x).tone]}
                          pressed={k() === picked()}
                          onClick={() => setPicked(k())}
                        />
                      )}
                    </For>
                  </Wrap>
                </Show>
                <Stack gap="1">
                  <Link href={p().url} external tone="ink" title={p().title ?? ''} part="side.pr.title">
                    {p().title || t('shell-task.pr.untitled', { n: p().number })}
                  </Link>
                  <Text size="sm" tone="muted" truncate part="side.pr.meta">
                    {[
                      `#${p().number}`,
                      t(p().draft && p().state === 'OPEN' ? 'shell-task.pr.stateDraft' : (STATE[p().state] ?? 'shell-task.pr.open')),
                      p().branch ? `${p().branch} → ${p().base ?? ''}` : '',
                      p().author ? t('shell-task.pr.by', { name: p().author ?? '' }) : '',
                    ]
                      .filter(Boolean)
                      .join(' · ')}
                  </Text>
                </Stack>
                <Banner
                  part={`side.pr.verdict.${v().tone}`}
                  tone={TONE[v().tone]}
                  title={t(v().key, v().vars)}
                  {...((v().act === 'merge' && (list() ?? []).length === 1) || v().act === 'fix'
                    ? {
                        action: (
                          <Button
                            size="sm"
                            variant={v().primary ? 'primary' : 'ghost'}
                            icon={v().act === 'merge' ? 'check' : 'merge'}
                            onClick={() => (v().act === 'fix' ? quiet(c.run(fixConflicts, args())) : setMergeAsk(true))}
                          >
                            {t(v().act === 'fix' ? 'shell-task.pr.fix' : 'shell-task.pr.merge')}
                          </Button>
                        ),
                      }
                    : {})}
                />
                <Show when={p().error}>{(e) => <Banner tone="warn" title={e()} part="side.pr.error" />}</Show>
                <PrRisk url={p().url} open={p().state === 'OPEN'} />
                <Show when={p().checkList}>
                  <Stack gap="1" part="side.pr.checks">
                    <PaneHead title={t('shell-task.pr.checks')} {...((p().checkList ?? []).length > 0 && { meta: String((p().checkList ?? []).length) })} />
                    <Show
                      when={(p().checkList ?? []).length > 0}
                      fallback={
                        <Text size="sm" tone="muted">
                          {t('shell-task.pr.noChecks')}
                        </Text>
                      }
                    >
                      <CheckList label={t('shell-task.pr.checks')}>
                        <For each={[...g().bad, ...(g().okOpen || open().has('passed') ? g().ok : [])]}>
                          {(k) => (
                            <CheckRow
                              name={k.name}
                              state={k.state === 'skip' ? 'skipped' : (k.state as 'pass' | 'fail' | 'pending')}
                              duration={k.state === 'pending' ? t('shell-task.pr.running') : k.state === 'skip' ? t('shell-task.pr.skipped') : dur(k.ms)}
                              {...((k.url || (k.state === 'fail' && canAsk() && p().state === 'OPEN')) && {
                                actions: (
                                  <Inline gap="1">
                                    <Show when={k.state === 'fail' && canAsk() && p().state === 'OPEN'}>
                                      <Button size="xs" variant="ghost" icon="wizard" part="side.pr.askFix" loading={asking() === k.name} title={t('shell-task.pr.askFixTitle')} onClick={() => askFix(k, p().number)}>
                                        {t('shell-task.pr.askFix')}
                                      </Button>
                                    </Show>
                                    <Show when={k.url}>
                                      <IconButton icon="external" size="xs" label={t('shell-task.pr.logs', { name: k.name })} onClick={() => window.open(k.url, '_blank', 'noopener')} />
                                    </Show>
                                  </Inline>
                                ),
                              })}
                            />
                          )}
                        </For>
                      </CheckList>
                      <Show when={g().ok.length > 0 && !g().okOpen}>
                        <FoldRow
                          part="side.pr.fold"
                          icon="check"
                          tone="good"
                          expanded={open().has('passed')}
                          onClick={() => (open().has('passed') ? fold('passed') : unfold('passed'))}
                          label={
                            open().has('passed')
                              ? t('shell-task.pr.foldPassed', { count: g().ok.length })
                              : [g().passed && t('shell-task.pr.passed', { count: g().passed }), g().skipped && t('shell-task.pr.skippedN', { count: g().skipped })].filter(Boolean).join(' · ')
                          }
                        />
                      </Show>
                    </Show>
                  </Stack>
                </Show>
                <Show when={(p().reviewers ?? []).length > 0}>
                  <Stack gap="1" part="side.pr.reviews">
                    <PaneHead title={t('shell-task.pr.reviews')} />
                    <For each={p().reviewers}>
                      {(r) => (
                        <Inline gap="2" justify="between">
                          {who(r)}
                          <Text size="sm" tone={r.state === 'APPROVED' ? 'good' : 'warn'} nowrap>
                            {t(`shell-task.pr.review.${r.state}`)}
                          </Text>
                        </Inline>
                      )}
                    </For>
                  </Stack>
                </Show>
                <Show when={said().live.length + said().outdated.length + said().people.length + said().bots.length > 0}>
                  <Stack gap="2" part="side.pr.talk">
                    <PaneHead title={t('shell-task.pr.talk')} />
                    <Stack gap="3" as="ul">
                      <For each={[...said().live, ...(open().has('outdated') ? said().outdated : [])]}>
                        {(th) => (
                          <Stack gap="1" as="li" part="side.pr.thread">
                            <Text size="xs" tone="muted" mono truncate>{`${th.path}${th.line ? `:${th.line}` : ''}`}</Text>
                            {note(th.notes[0] as PrNote)}
                          </Stack>
                        )}
                      </For>
                      <For each={[...said().people, ...(open().has('bots') ? said().bots : [])]}>{(n) => note(n)}</For>
                    </Stack>
                    <Inline gap="2">
                      <Show when={said().outdated.length > 0 && !open().has('outdated')}>
                        <FoldRow label={t('shell-task.pr.outdated', { count: said().outdated.length })} onClick={() => unfold('outdated')} />
                      </Show>
                      <Show when={said().bots.length > 0 && !open().has('bots')}>
                        <FoldRow label={t('shell-task.pr.bots', { count: said().bots.length })} onClick={() => unfold('bots')} />
                      </Show>
                    </Inline>
                  </Stack>
                </Show>
                <Show when={files().shown.length > 0}>
                  <Stack gap="1" part="side.pr.files">
                    <PaneHead title={t('shell-task.pr.files')} meta={`+${p().additions ?? 0} −${p().deletions ?? 0}`} />
                    <FileStatList label={t('shell-task.pr.files')}>
                      <For each={files().shown}>{(f) => <FileStatRow path={f.path} additions={f.additions} deletions={f.deletions} href={`${p().url}/files`} />}</For>
                    </FileStatList>
                    <Show when={files().more > 0}>
                      <FoldRow label={t('shell-task.pr.more', { count: files().more })} onClick={() => unfold('files')} />
                    </Show>
                  </Stack>
                </Show>
                <Inline gap="2" justify="between" part="side.pr.foot">
                  <Link href={p().url} external tone="muted">
                    {t('shell-task.pr.github')}
                  </Link>
                  <Inline gap="1">
                    <Text size="xs" tone="muted" nowrap>
                      {t('shell-task.pr.checked', { when: timeLabel(at() || Date.now(), Date.now(), kitT) })}
                    </Text>
                    <IconButton icon="replay" size="xs" label={t('shell-task.pr.checkNow')} part="side.pr.check" onClick={() => quiet(c.run(refresh, args()))} />
                  </Inline>
                </Inline>
              </>
            );
          }}
        </Show>
      </Show>
      <Show when={mergeAsk()}>
        <Dialog
          open
          role="alertdialog"
          size="sm"
          title={t('shell-task.pr.mergeAsk.title')}
          description={t('shell-task.pr.mergeAsk.body')}
          onOpenChange={(o) => !o && setMergeAsk(false)}
          part="side.pr.mergeAsk"
          footer={
            <Inline gap="4" justify="end">
              <Button variant="ghost" onClick={() => setMergeAsk(false)}>
                {t('shell-task.pr.mergeAsk.cancel')}
              </Button>
              <Button variant="primary" icon="check" part="side.pr.mergeAsk.ok" onClick={doMerge}>
                {t('shell-task.pr.merge')}
              </Button>
            </Inline>
          }
        />
      </Show>
    </Stack>
  );
}
