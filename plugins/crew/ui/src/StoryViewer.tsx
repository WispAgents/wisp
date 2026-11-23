// The full-screen story overlay opened when a crew ring is tapped (lazy chunk: never loaded on the phone first screen
// until a ring is tapped, ADR-0105).
import { createMemo, createSignal, lazy, Show, type Accessor, type JSX } from 'solid-js';
import { uuidv7, type EntityKey, type PluginClient } from '@wisp/client';
import { ExtView, useExtensions, useShell } from '@wisp/shell';
import { Button, CrewStory, Dialog, Face, StoryCard, StoryText, timeLabel, englishT, useSignal, Wrap, type MenuEntry } from '@wisp/ui';
import { assignJob, checkIn, followJob, removeMember, updateMember } from './commands.ts';
import { emptyState, type CrewData, type CrewMember } from './data.ts';
import { t } from './i18n.ts';
import type { State } from './model.ts';
import { askJob, lastReport, nextLine, nowJob, replyTarget, stepStory } from './stories.ts';
import type { SessionRow } from './types.ts';
import { VIEW } from './views.ts';
import './i18n/lazy.ts';

// the story's reply box: its own chunk, loaded as a story opens (the task page's kit chunk, perf r1 #1)
const ReplyBox = lazy(() => import('./StoryReply.tsx'));

type Say = (k: string, v?: Readonly<Record<string, string | number>>) => string;
const say = t as Say;
const bare = (key: string): string => key.slice(key.indexOf(':') + 1);

export interface StoryViewerProps {
  readonly client: PluginClient;
  readonly data: CrewData;
  readonly order: Accessor<readonly CrewMember[]>;
  readonly states: Accessor<ReadonlyMap<string, State>>;
  readonly held: Accessor<readonly string[]>;
  readonly open: Accessor<string | null>;
  readonly setOpen: (id: string | null) => void;
}

// size-ok: the open story overlay, its menu actions and remove confirmation share the open member state
export default function StoryViewer(props: StoryViewerProps): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const nav = useSignal(shell.nav);
  const stateOf = (m: CrewMember): State => props.states().get(m.id) ?? emptyState(!!m.paused);
  // the task page's answers only: shell-task's foot (its AskCard), not the slot's other extensions (Terminal, Watch,
  // the comments tray), which belong to an open task and would open its side panes from here
  const feet = useExtensions('shell.task:foot');
  const askFoot = createMemo(() => feet().find((e) => e.id === 'shell-task.foot'));
  const crew = createMemo(() => props.held().flatMap((id) => props.order().find((m) => m.id === id) ?? []));
  const at = createMemo(() => crew().findIndex((m) => m.id === props.open()));
  const member = createMemo(() => crew()[at()]);
  const close = (): void => props.setOpen(null);
  // a draft per member: stepping on keeps what you typed for each
  const [drafts, setDrafts] = createSignal<ReadonlyMap<string, string>>(new Map());
  const draft = (id: string): string => drafts().get(id) ?? '';
  const setDraft = (id: string, v: string): void => void setDrafts(new Map(drafts()).set(id, v));
  const [removing, setRemoving] = createSignal(false);
  const now = (): number => c.scope.timers.now();
  const when = (ts: number): string => timeLabel(ts, now(), englishT);
  const openTask = (s: SessionRow): void => {
    const key = props.data.sessionKey(s.id);
    if (!key) return;
    close();
    shell.navigate({ t: 'openTask', id: key as never });
  };
  /** a follow-up on a job, at once; refused, the text comes back (and the shell's toast says why) */
  const follow = (s: SessionRow, text: string, back?: () => void): void => {
    const key = props.data.sessionKey(s.id);
    if (!key) return back?.();
    void c.run(followJob, { key: key as EntityKey, sessionId: bare(key), text }).catch(() => back?.());
  };
  const send = (m: CrewMember): void => {
    const text = draft(m.id).trim();
    if (!text) return;
    setDraft(m.id, '');
    const back = (): void => setDraft(m.id, text);
    const target = replyTarget(stateOf(m));
    if (target.kind === 'follow') return follow(target.job, text, back);
    void c.run(assignJob, { key: m.key, crewId: m.id, job: uuidv7(), prompt: text, title: text.split('\n')[0]?.slice(0, 80) ?? text }).catch(back);
  };
  const menu = (m: CrewMember): MenuEntry[] => [
    { value: 'page', label: say('wisp.crew.stories.openPage', { name: m.name }), icon: 'crew' },
    { value: 'edit', label: t('wisp.crew.page.editMember'), icon: 'pen' },
    { value: 'checkin', label: t('wisp.crew.page.checkin'), icon: 'clock' },
    { value: 'pause', label: t(m.paused ? 'wisp.crew.page.resume' : 'wisp.crew.page.pause'), icon: m.paused ? 'play' : 'pause' },
    { value: 'share', label: t('wisp.crew.page.share'), icon: 'share' },
    { value: 'shareCrew', label: t('wisp.crew.page.shareCrew'), icon: 'crew' },
    { value: 'record', label: t('wisp.crew.page.record'), icon: 'stats' },
    { kind: 'separator' },
    { value: 'remove', label: t('wisp.crew.page.remove'), icon: 'trash', danger: true },
  ];
  const overlay = (id: string, params: unknown): void => c.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: id as never, params: params as never } });
  const pick = (m: CrewMember, v: string): void => {
    if (v === 'page') {
      close();
      c.navigate({ t: 'openCrew', id: m.key as never });
    } else if (v === 'edit') overlay(VIEW.editor, { id: m.id });
    else if (v === 'checkin') void c.run(checkIn, { key: m.key, crewId: m.id }).catch(() => undefined);
    else if (v === 'pause') void c.run(updateMember, { key: m.key, id: m.id, paused: !m.paused }).catch(() => undefined);
    else if (v === 'share') overlay(VIEW.share, { ids: [m.id], name: m.name });
    else if (v === 'shareCrew') overlay(VIEW.share, {});
    else if (v === 'record') overlay(VIEW.record, { id: m.id });
    else if (v === 'remove') setRemoving(true);
  };
  const remove = (m: CrewMember): void => {
    setRemoving(false);
    close();
    void c.run(removeMember, { key: m.key, id: m.id }).catch(() => undefined);
  };
  return (
    <Show when={member()}>
      {(m) => {
        const st = (): State => stateOf(m());
        const ask = (): SessionRow | undefined => askJob(st());
        const job = (): SessionRow | undefined => nowJob(st());
        const report = (): SessionRow | undefined => lastReport(st());
        const idle = (): boolean => replyTarget(st()).kind === 'assign';
        return (
          <>
            <CrewStory
              title={say('wisp.crew.stories.title', { name: m().name })}
              index={at()}
              count={crew().length}
              onStep={(to) => props.setOpen(crew()[stepStory(to, 0, crew().length)]?.id ?? null)}
              onClose={close}
              hold={removing() || nav().overlays.length > 0}
              face={<Face avatar={m().avatar} name={m().name} color={m().color} size="md" mood={st().mood} dim={m().paused} />}
              name={m().name}
              sub={[m().role, job()?.title].filter(Boolean).join(' · ')}
              menu={{ label: say('wisp.crew.stories.more', { name: m().name }), items: menu(m()), onSelect: (v) => pick(m(), v) }}
              labels={{
                close: t('wisp.crew.stories.close'),
                prev: t('wisp.crew.stories.prev'),
                next: t('wisp.crew.stories.next'),
                pause: t('wisp.crew.stories.pause'),
                resume: t('wisp.crew.stories.resume'),
                where: say('wisp.crew.stories.where', { n: at() + 1, count: crew().length }),
              }}
              reply={
                <ReplyBox
                  compact
                  part="crew.story.composer"
                  placeholder={say(idle() ? 'wisp.crew.stories.give' : 'wisp.crew.stories.message', { name: m().name })}
                  value={draft(m().id)}
                  onInput={(v) => setDraft(m().id, v)}
                  onSend={() => send(m())}
                  send={t('wisp.crew.stories.send')}
                />
              }
            >
              <Show when={ask()}>
                {(s) => (
                  <StoryCard label={t('wisp.crew.stories.waiting')} tone="bad" part="crew.story.waiting">
                    {/* the task page's own question and answers (shell-task's foot over its reply box), as they are there */}
                    <Show when={askFoot()}>{(ext) => <ExtView ext={ext()} id={props.data.sessionKey(s().id) ?? s().id} session={() => s() as never} />}</Show>
                    <Wrap gap="4" part="crew.story.waiting.acts">
                      <Button size="sm" variant="secondary" icon="expand" onClick={() => openTask(s())}>
                        {t('wisp.crew.stories.openTask')}
                      </Button>
                    </Wrap>
                  </StoryCard>
                )}
              </Show>
              <StoryCard label={t('wisp.crew.stories.now')} part="crew.story.now">
                <Show when={job()} fallback={<StoryText title={t('wisp.crew.stories.idle')} line={t('wisp.crew.stories.noJob')} />}>
                  {(s) => (
                    <StoryText
                      title={s().title}
                      line={s().state === 'waiting' ? t('wisp.crew.stories.paused') : s().live || t('wisp.crew.page.starting')}
                      live={s().state !== 'waiting'}
                    />
                  )}
                </Show>
              </StoryCard>
              <StoryCard label={t('wisp.crew.stories.last')} part="crew.story.report">
                <Show when={report()} fallback={<StoryText line={t('wisp.crew.stories.noReport')} />}>
                  {(s) => (
                    <>
                      <StoryText line={s().preview || s().title} />
                      <Wrap gap="4" part="crew.story.report.acts">
                        <Button size="sm" variant="secondary" icon="thumb-up" part="crew.story.good" onClick={() => follow(s(), t('wisp.crew.stories.good'))}>
                          {t('wisp.crew.stories.good')}
                        </Button>
                        <Button size="sm" variant="secondary" icon="rerun" part="crew.story.again" onClick={() => follow(s(), t('wisp.crew.stories.again'))}>
                          {t('wisp.crew.stories.again')}
                        </Button>
                        <Button size="sm" variant="secondary" icon="expand" part="crew.story.openReport" onClick={() => openTask(s())}>
                          {t('wisp.crew.stories.openReport')}
                        </Button>
                      </Wrap>
                    </>
                  )}
                </Show>
              </StoryCard>
              <StoryCard label={t('wisp.crew.stories.next.label')} part="crew.story.checkin">
                <StoryText line={nextLine(m(), { t: say as never, now: now(), when })} />
              </StoryCard>
            </CrewStory>
            <Dialog
              open={removing()}
              onOpenChange={setRemoving}
              size="sm"
              role="alertdialog"
              title={say('wisp.crew.page.remove.confirm', { name: m().name })}
              description={t('wisp.crew.page.remove.why')}
              footer={
                <>
                  <Button variant="secondary" size="sm" onClick={() => setRemoving(false)}>
                    {t('wisp.crew.page.cancel')}
                  </Button>
                  <Button variant="danger" size="sm" onClick={() => remove(m())}>
                    {say('wisp.crew.page.remove.ok', { name: m().name })}
                  </Button>
                </>
              }
            />
          </>
        );
      }}
    </Show>
  );
}
