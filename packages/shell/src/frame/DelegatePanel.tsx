// Delegate (v1 team.js teamEditor): who the lead can hand parts of the task to. The head says what it is; the lead's
// strip says who leads, on what and where; a row per delegate: where it runs (with the lead or a machine), its
// harness · model · thinking (the same picker New task uses), its copies and Remove, then the @name the lead calls it
// and what it's for; or the empty note. The foot: Add delegate (asking where when there are machines), how many
// agents in all, Clear. Controlled: every change goes to onChange. Loaded with the panel (lazy), not the first page.
// Not yet here: v1's Presets and Save as preset (v2 keeps no team presets), crew members as delegates.

import { Index, Show, createSignal, type JSX } from 'solid-js';
import type { PeerId } from '@wisp/client';
import { AutoRunner, Button, IconButton, Menu, TeamCopies, TeamFields, TeamLead, TeamPop, TeamRow, TeamWhere, useSignal, type Act, type IconName, type MenuEntry, type TeamPlaceLook } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { RunnerPicker } from './RunnerPicker.tsx';
import { useHarnessRows } from './runner.ts';
import { MAX_COPIES, copiesOf, nextAgent, teamSize, uniqueNames, type Delegate } from './team.ts';
import '../i18n/lazy.ts';

/** A place a delegate can run away from the lead: this computer (`here`) or a machine (its peer id). */
export interface DelegatePlace {
  readonly id: string;
  readonly label: string;
  readonly icon: IconName;
  readonly off?: boolean;
}

export interface DelegatePanelProps {
  readonly lead: { readonly agent: string; readonly run: string; readonly logo?: string | undefined; readonly place: TeamPlaceLook; readonly placeAct?: Act | undefined };
  readonly team: readonly Delegate[];
  readonly onChange: (team: Delegate[]) => void;
  /** whose harnesses the rows pick from */
  readonly peer: string;
  /** where else a delegate can run (none: every row is with the lead, Add adds at once) */
  readonly places: readonly DelegatePlace[];
  /** Add a machine… (Settings → Machines) */
  readonly onAddMachine?: (() => void) | undefined;
}

const LEAD = 'lead';
const ADD_MACHINE = 'add-machine';

// size-ok: one panel's markup; its handlers are one-liners over the same list
export function DelegatePanel(props: DelegatePanelProps): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const defs = useSignal(shell.harnesses());
  const rows = useHarnessRows(shell.client.entities, () => props.peer);
  // installed ones in their order; before the topic answers, every harness a plugin registered
  const ids = (): string[] => {
    const found = rows()
      .filter((r) => r.state === 'installed')
      .sort((a, b) => a.ord - b.ord)
      .map((r) => r.id);
    return found.length ? found : defs().map((h) => h.id);
  };
  const named = () => uniqueNames(props.team);
  const set = (i: number, next: Partial<Delegate>): void => props.onChange(props.team.map((m, k) => (k === i ? { ...m, ...next } : m)));
  const add = (machine = ''): void => {
    const agent = nextAgent(props.lead.agent, props.team.filter((m) => (m.machine ?? '') === machine), ids());
    if (!agent) return void shell.toast.show(t('shell.team.noHarness'), { tone: 'error' });
    props.onChange([...props.team, { name: '', agent, use: '', ...(machine && { machine }) }]);
  };
  const placeLook = (m: Delegate): TeamPlaceLook => {
    const p = m.machine ? props.places.find((x) => x.id === m.machine) : undefined;
    if (!m.machine) return { icon: 'agent', label: t('shell.team.withLead'), title: t('shell.team.withLeadTip') };
    const label = p?.label ?? m.machine;
    return { icon: p?.icon ?? 'server', label, title: t('shell.team.awayTip', { name: label }), away: true, ...(p?.off && { off: true }) };
  };
  const whereItems = (current: string | undefined, adding: boolean): MenuEntry[] => [
    {
      kind: 'group',
      id: 'where',
      label: t(adding ? 'shell.team.addAt' : 'shell.team.runsOn'),
      items: [
        { value: LEAD, label: t('shell.team.withLead'), hint: t('shell.team.withLeadSub'), icon: 'agent', ...(!adding && { checked: !current }) },
        ...props.places.map((p) => ({ value: p.id, label: p.label, hint: t('shell.team.awaySub'), icon: p.icon, ...(!adding && { checked: current === p.id }) })),
      ],
    },
    ...(props.onAddMachine ? [{ kind: 'separator' as const }, { value: ADD_MACHINE, label: t(props.places.length ? 'shell.team.addMachine' : 'shell.team.addFirstMachine'), icon: 'plus' as const }] : []),
  ];
  const where = (v: string, then: (machine: string) => void): void => (v === ADD_MACHINE ? props.onAddMachine?.() : then(v === LEAD ? '' : v));
  const n = (): number => teamSize(props.team);
  return (
    <TeamPop
      part="delegate.pop"
      wide
      title={t('shell.team.title')}
      intro={t('shell.team.intro')}
      lead={<TeamLead part="delegate.lead" tag={t('shell.team.lead')} logo={props.lead.logo} run={props.lead.run} on={t('shell.team.on')} place={props.lead.place} placeAct={props.lead.placeAct} />}
      count={props.team.length}
      empty={{ icon: 'agent', text: t('shell.team.empty') }}
      add={
        <Show when={props.places.length > 0 || props.onAddMachine} fallback={<Button variant="ghost" size="sm" icon="plus" part="delegate.add" onClick={() => add()}>{t('shell.team.add')}</Button>}>
          <Menu part="delegate.add" trigger={{ label: t('shell.team.add'), icon: 'plus', variant: 'ghost', size: 'sm' }} items={whereItems(undefined, true)} onSelect={(v) => where(v, add)} />
        </Show>
      }
      tally={t(n() === 1 ? 'shell.team.count.one' : 'shell.team.count.other', { count: n() })}
      clear={props.team.length ? { label: t('shell.team.clear'), onClick: () => props.onChange([]) } : undefined}
    >
      {/* by place, not by object: a row keeps its node (and the field you type in) while its words change */}
      <Index each={props.team}>
        {(m, i) => (
          <TeamRow
            part="delegate.row"
            remove={
              <>
                <TeamCopies part="delegate.copies" n={copiesOf(m())} max={MAX_COPIES} title={t('shell.team.copies')} fewer={t('shell.team.fewer')} more={t('shell.team.more')} onChange={(count) => set(i, { count })} />
                <IconButton icon="x" label={t('shell.team.remove')} variant="ghost" size="sm" part="delegate.remove" onClick={() => props.onChange(props.team.filter((_, k) => k !== i))} />
              </>
            }
            bottom={
              <TeamFields
                part="delegate.fields"
                name={{ value: m().name, placeholder: named()[i]?.name ?? m().agent, title: t('shell.team.nameTitle'), onInput: (name) => set(i, { name }) }}
                use={{ label: t('shell.team.use'), value: m().use ?? '', placeholder: t('shell.team.usePlaceholder'), onInput: (use) => set(i, { use }) }}
              />
            }
          >
            <TeamWhere
              part="delegate.place"
              place={placeLook(m())}
              act={{
                menu: {
                  items: whereItems(m().machine, false),
                  onSelect: (v) =>
                    where(v, (machine) => {
                      const { machine: _was, ...rest } = m();
                      props.onChange(props.team.map((x, k) => (k === i ? (machine ? { ...rest, machine } : rest) : x)));
                    }),
                },
              }}
            />
            <DelegateRunner peer={((x) => (x && x !== 'here' && x !== 'sandbox' ? x : props.peer))(m().machine)} m={m()} onChange={(r) => set(i, r)} />
          </TeamRow>
        )}
      </Index>
    </TeamPop>
  );
}

/** A delegate's harness, model and thinking (v1 .tp-runner): one button, the picker New task uses under it. */
function DelegateRunner(props: { readonly peer: string; readonly m: Delegate; readonly onChange: (r: Partial<Delegate>) => void }): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const defs = useSignal(shell.harnesses());
  const rows = useHarnessRows(shell.client.entities, () => props.peer);
  const [open, setOpen] = createSignal(false);
  let btn: HTMLButtonElement | undefined;
  const def = () => defs().find((h) => h.id === props.m.agent);
  const model = (): string => {
    if (props.m.modelLabel) return props.m.modelLabel;
    const offer = rows().find((r) => r.id === props.m.agent)?.agent;
    return offer?.models.find((x) => x.id === props.m.model)?.label ?? (props.m.model || t('shell.team.defaultModel'));
  };
  return (
    <>
      <AutoRunner
        part="delegate.runner"
        ref={(el) => (btn = el)}
        harness={def()?.name ?? props.m.agent}
        model={[model(), props.m.effort].filter(Boolean).join(' · ')}
        logo={def()?.logo}
        title={t('shell.team.runner')}
        onClick={() => setOpen(!open())}
      />
      <RunnerPicker
        open={open()}
        onOpenChange={setOpen}
        anchor={() => btn}
        peer={props.peer as PeerId}
        value={{ agent: props.m.agent, model: props.m.model ?? '', effort: props.m.effort ?? '', account: '', ...(props.m.modelLabel && { modelLabel: props.m.modelLabel }) }}
        onChange={(r) => props.onChange({ agent: r.agent, model: r.model, effort: r.effort, ...(r.modelLabel ? { modelLabel: r.modelLabel } : { modelLabel: '' }) })}
      />
    </>
  );
}

export default DelegatePanel;
