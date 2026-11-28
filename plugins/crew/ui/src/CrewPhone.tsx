// The Crew tab in the phone apps (ADR-0065 step 14, design/mobile.md §A.2 Crew): the full place for the crew, where the
// stories under the Tasks bar (Stories.tsx) are the glance. Built from the kit, so each platform's look is the kit's:
// - the root: the platform's root bar titled Crew with Org and Add member (+), its ⋮ / ⋯ holding Import, Share your
//   crew and Track record; the members as M3 two-line list items with their faces / an iOS inset grouped list, most
//   needed first (asks you › new report › working › idle), each pushing its page;
// - a member's page: the page bar with Back to Crew, Pause / Resume and its ⋮ / ⋯ (Edit, Check in now, Org, Share,
//   Share your crew, Track record, Remove); its face, role and what it's doing; M3 primary tabs / an iOS segmented
//   control for Jobs · Brief · Wakes; and New job, the task page's reply box (compact) docked under it;
// - Org (the chart, F-crew-14) pushed from the root with Back.
// Every write is the browser page's own command (crew.update, crew.assign, crew.checkIn, crew.remove): it shows at once
// and rolls back with the shell's toast when refused. Its own chunk: only the phone apps load it.
import { createMemo, createSignal, For, Show, type JSX } from 'solid-js';
import { uuidv7, type PluginClient } from '@wisp/client';
import { useHere, useShell } from '@wisp/shell';
import {
  Box,
  Button,
  CrewFacts,
  CrewText,
  Dialog,
  EmptyState,
  englishT,
  Face,
  IconButton,
  ListRow,
  Pill,
  ReplyBox,
  Scroll,
  Section,
  Stack,
  Tabs,
  TextArea,
  timeLabel,
  TopAppBar,
  useSignal,
  Wrap,
  type PartName,
  type ToolbarItem,
} from '@wisp/ui';
import { assignJob, checkIn, removeMember, updateMember, type MemberPatch } from './commands.ts';
import { crewData, emptyState, type CrewMember } from './data.ts';
import { t } from './i18n.ts';
import type { State } from './model.ts';
import { facts } from './page.ts';
import { storyOrder } from './stories.ts';
import { isTab, jobRows, memberMenu, memberRow, rootMenu, TABS, wakeLines, type PhoneTab } from './phone.ts';
import { KitSection } from './more/index.ts';
import type { SessionRow } from './types.ts';
import { VIEW } from './views.ts';
import { useHarnessLabel } from './harness.ts';
import './i18n/lazy.ts';

type Say = (k: string, v?: Readonly<Record<string, string | number>>) => string;
const say = t as Say;

const overlay = (c: PluginClient, id: string, params: unknown): void =>
  c.navigate({
    t: 'openOverlay',
    overlay: { kind: 'dialog', id: id as never, params: params as never },
  });
const toOrg = (c: PluginClient, chart: boolean): void =>
  c.navigate({
    t: 'openView',
    id: VIEW.org as never,
    params: (chart ? { chart: true } : null) as never,
  });
const bar = (id: string, label: string, icon: 'plus' | 'org' | 'play' | 'pause', part: PartName, run: () => void): ToolbarItem => ({
  id,
  label,
  icon,
  priority: 'pin',
  onSelect: run,
  render: () => <IconButton icon={icon} label={label} onClick={run} part={part} />,
});

function useCtx(c: PluginClient): {
  now: () => number;
  when: (at: number) => string;
} {
  const now = (): number => c.scope.timers.now();
  return { now, when: (at) => timeLabel(at, now(), englishT) };
}

/** The Crew tab's root: the members, most needed first, each pushing its page. */
export function CrewPhoneRoot(props: { readonly client: PluginClient }): JSX.Element {
  const c = props.client;
  const data = crewData(c);
  const ranked = useSignal(data.ranked);
  const states = useSignal(data.states);
  const { now, when } = useCtx(c);
  const stateOf = (m: CrewMember): State => states().get(m.id) ?? emptyState(!!m.paused);
  // the stories row's order (asks you › new report › working › idle), so the Tasks glance and this list agree
  const order = createMemo(() => storyOrder(ranked(), states()));
  const add = (): void => overlay(c, VIEW.editor, {});
  const pick = (v: string): void => {
    if (v === 'import') overlay(c, VIEW.import, {});
    else if (v === 'shareCrew') overlay(c, VIEW.share, {});
    else if (v === 'record') overlay(c, VIEW.record, {});
  };
  // keyed by id: a row stays the same node while what it says follows its member (AGENTS.md "Nothing jumps")
  const ids = createMemo(() => order().map((m) => m.id), undefined, {
    equals: (a, b) => a.length === b.length && a.every((x, i) => x === b[i]),
  });
  const byId = createMemo(() => new Map(order().map((m) => [m.id, m] as const)));
  return (
    <>
      <TopAppBar
        kind="root"
        title={t('wisp.crew.phone.title')}
        part="crew.phone.bar"
        actions={[bar('org', t('wisp.crew.org.title'), 'org', 'crew.phone.org', () => toOrg(c, true)), bar('add', t('wisp.crew.phone.add'), 'plus', 'crew.phone.add', add)]}
        menu={{
          label: t('wisp.crew.phone.more'),
          items: rootMenu({ t }, ranked().length > 0),
          onSelect: pick,
        }}
      />
      <Scroll grow part="crew.phone" aria-label={t('wisp.crew.phone.title')}>
        <Show
          when={ids().length > 0}
          fallback={
            <EmptyState
              icon="crew"
              title={t('wisp.crew.phone.empty')}
              action={
                <Button variant="primary" icon="plus" part="crew.phone.empty-add" onClick={add}>
                  {t('wisp.crew.phone.add')}
                </Button>
              }
            >
              {t('wisp.crew.phone.emptyHow')}
            </EmptyState>
          }
        >
          <Section title={say('wisp.crew.phone.members', { count: ids().length })} part="crew.phone.members">
            <For each={ids()}>
              {(id) => {
                const m = (): CrewMember | undefined => byId().get(id);
                return (
                  <Show when={m()}>
                    {(mm) => {
                      const row = () => memberRow(mm(), stateOf(mm()), { t, now: now(), when });
                      return (
                        <ListRow
                          part="crew.phone.member"
                          title={mm().name}
                          sub={row().sub}
                          push
                          lead={<Face avatar={mm().avatar} name={mm().name} color={mm().color} size="md" mood={stateOf(mm()).mood} dim={mm().paused} />}
                          pills={
                            <Show when={row().pill}>
                              {(p) => (
                                <Pill tone={p().tone} dot live={p().tone === 'info'} variant="plain" size="sm">
                                  {p().label}
                                </Pill>
                              )}
                            </Show>
                          }
                          onOpen={() => c.navigate({ t: 'openCrew', id: mm().key as never })}
                        />
                      );
                    }}
                  </Show>
                );
              }}
            </For>
          </Section>
        </Show>
      </Scroll>
    </>
  );
}

/** Org pushed from the Crew root: the chart as the browser draws it, under the page bar with Back. */
export function CrewPhoneOrg(props: { readonly client: PluginClient; readonly children: JSX.Element }): JSX.Element {
  return (
    <>
      <TopAppBar
        kind="page"
        title={t('wisp.crew.org.title')}
        part="crew.phone.org-bar"
        back={{
          label: t('wisp.crew.phone.title'),
          onBack: () => toOrg(props.client, false),
        }}
      />
      <Scroll grow part="crew.phone.org-page" aria-label={t('wisp.crew.org.title')}>
        {props.children}
      </Scroll>
    </>
  );
}

// size-ok: one page whose tabs share the member, its jobs and the New job draft
/** A member's page, pushed from the Crew root (or a story's Open page). */
export function CrewPhoneMember(props: { readonly client: PluginClient; readonly id: string }): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const here = useHere();
  const data = crewData(c);
  const members = useSignal(data.members);
  const states = useSignal(data.states);
  const harnesses = useSignal(shell.harnesses());
  const { now, when } = useCtx(c);
  const member = createMemo(() => (members(), data.byId(props.id)));
  const [tab, setTab] = createSignal<PhoneTab>('jobs');
  const [draft, setDraft] = createSignal('');
  const [removing, setRemoving] = createSignal(false);
  const [editing, setEditing] = createSignal<'notebook' | 'brief' | undefined>();
  const [text, setText] = createSignal('');
  const back = (): void => toOrg(c, false);
  // a member with no harness of its own runs as New task does while Auto is off (audit A #22)
  const harness = useHarnessLabel(c, harnesses);
  const open = (s: Pick<SessionRow, 'id'>): void => {
    const key = data.sessionKey(s.id);
    if (key) shell.navigate({ t: 'openTask', id: key as never });
  };
  return (
    <Show
      when={member()}
      fallback={
        <>
          <TopAppBar kind="page" title={t('wisp.crew.phone.title')} part="crew.phone.member-bar" back={{ label: t('wisp.crew.phone.title'), onBack: back }} />
          <EmptyState icon="crew" title={t('wisp.crew.page.missing')} compact />
        </>
      }
    >
      {(m) => {
        const st = (): State => states().get(m().id) ?? emptyState(!!m().paused);
        const row = () => memberRow(m(), st(), { t, now: now(), when });
        const jobs = createMemo(() => jobRows(st().jobs, { t }));
        // keyed by id: a row stays the same node as its job streams (only what it says changes)
        const jobIds = createMemo(() => jobs().map((j) => j.id), undefined, { equals: (a, b) => a.length === b.length && a.every((x, i) => x === b[i]) });
        const jobById = createMemo(() => new Map(jobs().map((j) => [j.id, j] as const)));
        const update = (patch: MemberPatch): void => void c.run(updateMember, { key: m().key, id: m().id, ...patch }).catch(() => undefined);
        const checkin = (): void => void c.run(checkIn, { key: m().key, crewId: m().id }).catch(() => undefined);
        const send = (): void => {
          const prompt = draft().trim();
          if (!prompt) return;
          setDraft(''); // the box empties at once; refused, the words come back (and the shell's toast says why)
          setTab('jobs');
          c.run(assignJob, {
            key: m().key,
            crewId: m().id,
            job: uuidv7(),
            prompt,
            title: prompt.split('\n')[0]?.slice(0, 80) ?? prompt,
          }).catch(() => draft() === '' && setDraft(prompt)); // refused: the words come back, unless you typed again
        };
        const remove = (): void => {
          setRemoving(false);
          back();
          void c.run(removeMember, { key: m().key, id: m().id }).catch(() => undefined);
        };
        const pick = (v: string): void => {
          if (v === 'edit') overlay(c, VIEW.editor, { id: m().id });
          else if (v === 'checkin') checkin();
          else if (v === 'pause') update({ paused: !m().paused });
          else if (v === 'org') toOrg(c, true);
          else if (v === 'share') overlay(c, VIEW.share, { ids: [m().id], name: m().name });
          else if (v === 'shareCrew') overlay(c, VIEW.share, {});
          else if (v === 'record') overlay(c, VIEW.record, { id: m().id });
          else if (v === 'remove') setRemoving(true);
        };
        const edit = (which: 'notebook' | 'brief'): void => {
          setText(m()[which] ?? '');
          setEditing(which);
        };
        const save = (): void => {
          const which = editing();
          if (which) update({ [which]: text() });
          setEditing(undefined);
        };
        const editor = (which: 'notebook' | 'brief'): JSX.Element => (
          <Section title={t(which === 'brief' ? 'wisp.crew.page.brief' : 'wisp.crew.page.notebook')} part={`crew.phone.${which}`}>
            <Stack gap="3" padX="7" padY="3">
              <TextArea
                value={text()}
                onInput={setText}
                minRows={4}
                maxRows={14}
                aria-label={t(which === 'brief' ? 'wisp.crew.page.brief' : 'wisp.crew.page.notebook')}
                part={`crew.phone.${which}.field`}
              />
              <Wrap gap="4">
                <Button variant="primary" size="sm" part={`crew.phone.${which}.save`} onClick={save}>
                  {t('wisp.crew.page.save')}
                </Button>
                <Button variant="secondary" size="sm" onClick={() => setEditing(undefined)}>
                  {t('wisp.crew.page.cancel')}
                </Button>
              </Wrap>
            </Stack>
          </Section>
        );
        const textSection = (which: 'notebook' | 'brief'): JSX.Element => (
          <Show
            when={editing() === which}
            fallback={
              <Section
                title={t(which === 'brief' ? 'wisp.crew.page.brief' : 'wisp.crew.page.notebook')}
                part={`crew.phone.${which}`}
                actions={
                  <Button variant="ghost" size="sm" part={`crew.phone.${which}.edit`} onClick={() => edit(which)}>
                    {t('wisp.crew.page.edit')}
                  </Button>
                }
              >
                <Box padX="7" padY="3">
                  <CrewText text={m()[which] || t(which === 'brief' ? 'wisp.crew.page.brief.empty' : 'wisp.crew.page.notebook.empty')} empty={!m()[which]} />
                </Box>
              </Section>
            }
          >
            {editor(which)}
          </Show>
        );
        return (
          <>
            <TopAppBar
              kind="page"
              title={m().name}
              part="crew.phone.member-bar"
              back={{ label: t('wisp.crew.phone.title'), onBack: back }}
              actions={[
                bar('pause', t(m().paused ? 'wisp.crew.page.resume' : 'wisp.crew.page.pause'), m().paused ? 'play' : 'pause', 'crew.phone.pause', () =>
                  update({ paused: !m().paused }),
                ),
              ]}
              menu={{
                label: say('wisp.crew.stories.more', { name: m().name }),
                items: memberMenu(m(), { t }),
                onSelect: pick,
              }}
            />
            <Scroll grow part="crew.phone.member" aria-label={m().name}>
              <ListRow
                part="crew.phone.head"
                title={m().role || t('wisp.crew.org.member')}
                sub={row().line}
                lead={<Face avatar={m().avatar} name={m().name} color={m().color} size="lg" mood={st().mood} dim={m().paused} />}
              />
              <Box padX="7" padY="3">
                <Tabs
                  part="crew.phone.tabs"
                  aria-label={say('wisp.crew.phone.tabs', { name: m().name })}
                  fill
                  items={TABS.map((id) => ({
                    value: id,
                    label: t(`wisp.crew.phone.tab.${id}`),
                    ...(id === 'jobs' && { count: jobs().length }),
                  }))}
                  value={tab()}
                  onChange={(v) => isTab(v) && setTab(v)}
                />
              </Box>
              <Show when={tab() === 'jobs'}>
                <Show
                  when={jobs().length > 0}
                  fallback={
                    <EmptyState icon="crew" title={say('wisp.crew.phone.noJobs', { name: m().name })} compact>
                      {t('wisp.crew.phone.noJobsHow')}
                    </EmptyState>
                  }
                >
                  <Section title={t('wisp.crew.phone.tab.jobs')} part="crew.phone.jobs">
                    <For each={jobIds()}>
                      {(id) => (
                        <Show when={jobById().get(id)}>
                          {(j) => (
                            <ListRow
                              part="crew.phone.job"
                              title={j().title}
                              sub={[j().origin, j().sub].filter(Boolean).join(' · ')}
                              strong={j().unread}
                              push
                              time={when(j().at)}
                              pills={
                                <Show when={j().pill}>
                                  {(p) => (
                                    <Pill tone={p().tone as never} dot live={p().tone === 'info'} variant="plain" size="sm">
                                      {p().label}
                                    </Pill>
                                  )}
                                </Show>
                              }
                              onOpen={() => open(j())}
                            />
                          )}
                        </Show>
                      )}
                    </For>
                  </Section>
                </Show>
              </Show>
              <Show when={tab() === 'brief'}>
                {textSection('brief')}
                {textSection('notebook')}
                <Section title={t('wisp.crew.phone.how')} part="crew.phone.facts">
                  <Box padX="7" padY="3">
                    <CrewFacts
                      items={facts(m(), {
                        t,
                        harness,
                        place: (id) => id,
                        ...(here.hub() && { here: here.name() }),
                        onTeam: () => toOrg(c, true),
                      })}
                    />
                  </Box>
                </Section>
                <Box padX="7" padY="3">
                  <KitSection client={c} member={m} />
                </Box>
              </Show>
              <Show when={tab() === 'wakes'}>
                <Section title={t('wisp.crew.phone.tab.wakes')} part="crew.phone.wakes">
                  <For each={wakeLines(m(), { t, now: now(), when })}>{(w) => <ListRow part={`crew.phone.wake.${w.id}`} title={w.title} sub={w.sub} />}</For>
                </Section>
                <Wrap gap="4" padX="7" padY="3" part="crew.phone.wakes.acts">
                  <Button variant="secondary" size="sm" icon="clock" part="crew.phone.checkin" onClick={checkin}>
                    {t('wisp.crew.page.checkin')}
                  </Button>
                  <Button variant="secondary" size="sm" icon="pen" part="crew.phone.schedule" onClick={() => overlay(c, VIEW.editor, { id: m().id })}>
                    {t('wisp.crew.phone.changeSchedule')}
                  </Button>
                </Wrap>
              </Show>
            </Scroll>
            <ReplyBox
              compact
              part="crew.phone.composer"
              placeholder={say('wisp.crew.stories.give', { name: m().name })}
              value={draft()}
              onInput={(v) => setDraft(v)}
              onSend={send}
              send={t('wisp.crew.phone.newJob')}
            />
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
                  <Button variant="danger" size="sm" part="crew.phone.remove-confirm" onClick={remove}>
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
