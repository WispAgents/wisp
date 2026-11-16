// Picking the judges (v1 judges.js judgesEditor): Met when All · Majority · Any with the rule in
// words, a row per judge (its harness, model and thinking level, or a crew member's face and role),
// Add judge (a menu with your crew when you have one), the count and Clear. Controlled: every change
// goes to onChange(judges, vote), named as the server keeps them. JudgesButton opens it from the
// launcher's Done when line and from Set / Edit goal.

import { For, Show, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { useShell } from '@wisp/shell';
import { Button, IconButton, Inline, Menu, Popover, Select, Stack, TeamCrew, TeamPop, TeamRow, Text, type MenuEntry } from '@wisp/ui';
import { Face } from './Faces.tsx';
import { t } from './i18n.ts';
import { isCrew, judgesLabel, named, nextHarness, ruleLine, VOTE_KEY } from './rules.ts';
import { useCrew, useHarnesses } from './task.ts';
import { MAX_JUDGES, VOTES, type HarnessJudge, type HarnessRow, type Judge, type Vote } from './types.ts';
import './i18n/lazy.ts';

export interface JudgesProps {
  readonly client: PluginClient;
  readonly judges: readonly Judge[];
  readonly vote: Vote;
  readonly onChange: (judges: Judge[], vote: Vote) => void;
}

const HINT: Readonly<Record<Vote, 'wisp.goal.vote.allHint' | 'wisp.goal.vote.majorityHint' | 'wisp.goal.vote.anyHint'>> = {
  all: 'wisp.goal.vote.allHint',
  majority: 'wisp.goal.vote.majorityHint',
  any: 'wisp.goal.vote.anyHint',
};

function HarnessRowEdit(props: { readonly j: HarnessJudge; readonly harnesses: readonly HarnessRow[]; readonly onChange: (j: HarnessJudge) => void }): JSX.Element {
  const h = (): HarnessRow | undefined => props.harnesses.find((x) => x.id === props.j.agent);
  const pickAgent = (agent: string): void => {
    const next = props.harnesses.find((x) => x.id === agent);
    const model = next?.model ?? next?.models[0]?.id ?? '';
    props.onChange({ agent, model, effort: next?.effort ?? next?.efforts?.[0] ?? '' });
  };
  return (
    <Inline gap="2">
      <Select aria-label={t('wisp.goal.judges.harness')} size="sm" items={props.harnesses.map((x) => ({ value: x.id, label: x.name, disabled: !x.available }))} value={props.j.agent} onChange={pickAgent} />
      <Select aria-label={t('wisp.goal.judges.model')} size="sm" items={(h()?.models ?? []).map((m) => ({ value: m.id, label: m.label }))} value={props.j.model} onChange={(model) => props.onChange({ ...props.j, model })} />
      <Show when={(h()?.efforts?.length ?? 0) > 0}>
        <Select aria-label={t('wisp.goal.judges.effort')} size="sm" items={(h()?.efforts ?? []).map((e) => ({ value: e, label: e }))} value={props.j.effort} onChange={(effort) => props.onChange({ ...props.j, effort })} />
      </Show>
    </Inline>
  );
}

// size-ok: one editor's markup, its handlers are one-liners over the same list
export function JudgesEditor(props: JudgesProps): JSX.Element {
  const crew = useCrew(props.client);
  const harnesses = useHarnesses(props.client);
  const shell = useShell();
  const commit = (list: readonly Judge[], vote: Vote = props.vote): void => props.onChange(named(list, crew(), harnesses()), vote);
  const crewFree = () => crew().filter((c) => !props.judges.some((j) => isCrew(j) && j.crew === c.id));
  const full = (): boolean => props.judges.length >= MAX_JUDGES;
  const addHarness = (): void => {
    const h = nextHarness(props.judges, harnesses());
    // v1 judges.js: said in a toast, the panel stays as it was
    if (!h) return void shell.toast.show(t('wisp.goal.judges.noHarness'), { tone: 'error' });
    const model = h.model ?? h.models[0]?.id ?? '';
    commit([...props.judges, { agent: h.id, model, effort: h.effort ?? h.efforts?.[0] ?? '' }]);
  };
  const addMenu = (): MenuEntry[] => [
    { value: 'harness', label: t('wisp.goal.judges.addHarness'), icon: 'agent', hint: t('wisp.goal.judges.addHarnessHint') },
    { kind: 'group', id: 'crew', label: t('wisp.goal.judges.yourCrew'), items: crewFree().map((c) => ({ value: `crew:${c.id}`, label: c.name, icon: 'crew' as const, ...(c.role && { hint: c.role }) })) },
  ];
  const onAdd = (v: string): void => (v.startsWith('crew:') ? commit([...props.judges, { crew: v.slice(5) }]) : addHarness());
  const rule = (): string => {
    const n = props.judges.length;
    if (n < 2) return t(n ? 'wisp.goal.judges.oneDecides' : 'wisp.goal.judges.addTwo');
    const r = ruleLine(props.vote, n);
    return r ? t(r.key, r.vars) : '';
  };
  const remove = (i: number): JSX.Element => <IconButton icon="x" label={t('wisp.goal.judges.remove')} variant="ghost" size="sm" onClick={() => commit(props.judges.filter((_, k) => k !== i))} />;
  return (
    <Stack gap="0" part="goal.judgesEditor">
      <TeamPop
        part="goal.judgesPop"
        title={t('wisp.goal.judges')}
        intro={t('wisp.goal.judges.intro')}
        vote={{
          label: t('wisp.goal.metWhen'),
          options: VOTES.map((v) => ({ value: v, label: t(VOTE_KEY[v]), hint: t(HINT[v]) })),
          value: props.vote,
          off: props.judges.length < 2,
          rule: rule(),
          onChange: (v) => commit(props.judges, v as Vote),
        }}
        count={props.judges.length}
        empty={{ icon: 'judge', text: t('wisp.goal.judges.empty') }}
        add={
          <Show when={crewFree().length > 0} fallback={<Button variant="ghost" size="sm" icon="plus" disabled={full()} onClick={addHarness}>{t('wisp.goal.judges.add')}</Button>}>
            <Menu trigger={{ label: t('wisp.goal.judges.add'), icon: 'plus', variant: 'ghost', size: 'sm' }} items={addMenu()} onSelect={onAdd} />
          </Show>
        }
        tally={t('wisp.goal.judges.count', { n: props.judges.length, max: MAX_JUDGES })}
        clear={props.judges.length ? { label: t('wisp.goal.judges.clear'), onClick: () => commit([]) } : undefined}
      >
        <For each={props.judges}>
          {(j, i) => (
            <Show
              when={isCrew(j) ? j : undefined}
              fallback={
                <TeamRow part="goal.judgeRow" remove={remove(i())}>
                  <HarnessRowEdit j={j as HarnessJudge} harnesses={harnesses()} onChange={(next) => commit(props.judges.map((x, k) => (k === i() ? next : x)))} />
                </TeamRow>
              }
            >
              {(c) => {
                const m = () => crew().find((x) => x.id === c().crew);
                return (
                  <TeamRow part="goal.judgeRow" crew remove={remove(i())}>
                    <TeamCrew
                      face={<Face client={props.client} who={c()} size="md" />}
                      name={m()?.name ?? c().name ?? t('wisp.goal.crewMember')}
                      sub={m() ? [m()?.role, t('wisp.goal.judges.ownHarness')].filter(Boolean).join(' · ') : t('wisp.goal.judges.gone')}
                    />
                  </TeamRow>
                );
              }}
            </Show>
          )}
        </For>
      </TeamPop>
    </Stack>
  );
}

/** The button: "Judges", "1 judge" or "3 judges · All", opening the editor in a popover. */
export function JudgesButton(props: JudgesProps & { readonly part?: `${string}.${string}`; readonly compact?: boolean }): JSX.Element {
  const label = (): string => {
    const w = judgesLabel(props.judges.length, props.vote);
    return t(w.key, w.vars);
  };
  return (
    <Popover
      part={props.part ?? 'goal.judgesButton'}
      bare
      trigger={props.compact ? { icon: 'judge', 'aria-label': label(), variant: 'ghost', size: 'sm' } : { icon: 'judge', label: label(), variant: props.judges.length ? 'secondary' : 'ghost', size: 'sm' }}
      placement="bottom-end"
    >
      <JudgesEditor client={props.client} judges={props.judges} vote={props.vote} onChange={props.onChange} />
    </Popover>
  );
}
