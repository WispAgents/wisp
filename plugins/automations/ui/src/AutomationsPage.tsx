// Automations as v1 opened it from ⌘K (features/automations.js openAutomations): Recipes and Triggers with their
// counts, each a list with "+ New …", and the trigger form v1 had (kind tiles, Name, Send to, Project, the kind's own
// rows, Run, Prompt, Mode, Harness, Cancel / Create trigger). Saved as a `trigger` resource (ADR-0043), the same
// declaration Settings → Automations, the CLI and formulas write. Each item has v1's actions: a recipe's Run (its
// fields asked in the run dialog), Run it on a schedule, Edit and Delete; a trigger's Run now / Check now, its on/off
// switch, Edit (the form on its spec) and Delete, each Delete asking first. Settings → Automations → Manage shows this
// same view, as v1's opened openAutomations in the settings pane (audit A #8, #9).
import { parsePick } from '@wisp/shell/pure';
import { createEffect, createMemo, createResource, createSignal, Match, on, onCleanup, onMount, Show, Switch, type JSX } from 'solid-js';
import type { PluginClient } from '@wisp/client';
import { RunnerPicker, cronWords, projectList, useShell } from '@wisp/shell';
import {
  AutoFoot,
  AutoList,
  AutoRunner,
  AutoNote,
  AutoPresets,
  AutoRow,
  AutomationsBody,
  Button,
  Dialog,
  englishT,
  IconButton,
  Input,
  Select,
  Switch as KitSwitch,
  TextArea,
  timeLabel,
  TriggerKinds,
  useSignal,
} from '@wisp/ui';
import { fireTrigger, recipes, removeRecipe, removeTrigger, saveTrigger, triggers, type Recipe, type Trigger } from './commands.ts';
import { fieldsOf } from './recipe.ts';
import { t } from './i18n.ts';
import {
  EVERY,
  HOOK_KINDS,
  KINDS,
  POLLED,
  SCHEDULE_PRESETS,
  TASK_KINDS,
  describeSpec,
  emptyForm,
  fireLabel,
  formOfSpec,
  keyOfSpec,
  kindOf,
  pick,
  readBack,
  specOfEdit,
  specOfForm,
  warning,
  type TriggerFormState,
} from './trigger-form.ts';
import './i18n/lazy.ts';

export const PAGE_VIEW = 'automations.page';
/** the recipes' rows (`automations_task`), as wisp.automations publishes them */
const RECIPES_TOPIC = 'plugin:wisp.automations:tasks' as const;

export interface PageParams {
  readonly tab: 'recipes' | 'triggers';
}

export default function AutomationsPage(props: { readonly client: PluginClient; readonly params: PageParams }): JSX.Element {
  const c = props.client;
  const peer = c.home;
  const projectFilter = useSignal(useShell().projectFilter?.value ?? ((): string | undefined => undefined)); // a new trigger starts in it
  const say = t as (k: string, v?: Readonly<Record<string, string | number>>) => string;
  const [tab, setTab] = createSignal<'recipes' | 'triggers'>(props.params.tab);
  const [rs, { refetch: again }] = createResource(() => c.run(recipes, { peer }).catch((): Recipe[] => []));
  // the recipes' topic: an edit or a new one (the recipe dialog), a run or a removal reads the list again
  const scope = c.scope.child('automations-page');
  onCleanup(() => scope.dispose());
  c.watch(peer, RECIPES_TOPIC, scope);
  const recipeRows = useSignal(c.entities.list<unknown>('automations_task'));
  createEffect(on(recipeRows, () => void again(), { defer: true }));
  // a crew member's own wakes (`from` wisp.crew) are the crew's: its page shows them, this list is the user's triggers
  const [ts, { refetch }] = createResource(() =>
    c
      .run(triggers, { peer })
      .then((l) => l.filter((x) => x.from === 'trigger'))
      .catch((): Trigger[] => []),
  );
  // the form: a new trigger (a draft, maybe from a recipe's Schedule), or an edit of one
  const [editing, setEditing] = createSignal<{ draft?: Partial<TriggerFormState>; old?: { name: string; spec: Readonly<Record<string, unknown>> } } | undefined>();
  const [deleting, setDeleting] = createSignal<{ what: 'recipe' | 'trigger'; name: string } | undefined>();
  const projectRows = useSignal(c.entities.list<unknown>('project'));
  const crew = useSignal(c.entities.list<{ id: string; name?: string; body?: { name?: string } }>('crew_member'));
  const harnesses = useSignal(useShell().harnesses());
  const place = (path: string): string => projectList(projectRows(), undefined).find((p) => p.path === path)?.name ?? path.split(/[\\/]/).pop() ?? path;
  const member = (id: string): string | undefined => {
    const m = (crew() ?? []).find((x) => x.id === id);
    return m ? (m.name ?? m.body?.name ?? m.id) : undefined;
  };
  const after = (p: Promise<unknown>, then: () => void): void => void p.then(then, () => {}); // a refusal toasts from the shell
  // the launcher's Recipes row does the same: its fields asked in a dialog, never the CLI's hint (audit A #12)
  const run = (r: Recipe): void => c.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'automations.run' as never, params: { name: r.name, project: r.project ?? '' } } });
  const openRecipe = (draft: Record<string, string> = { name: '', prompt: '' }): void =>
    c.navigate({ t: 'openOverlay', overlay: { kind: 'dialog', id: 'automations.recipe' as never, params: draft } });
  const edit = (r: Recipe): void =>
    openRecipe({ editing: r.name, name: r.name, prompt: r.prompt, project: r.project ?? '', mode: r.mode ?? '', agent: r.agent ?? '', model: r.model ?? '', effort: r.effort ?? '' });
  const toggle = (x: Trigger): void => {
    const spec: Record<string, unknown> = { ...(x.spec ?? {}) };
    if (x.state === 'off') delete spec['enabled'];
    else spec['enabled'] = false;
    after(c.run(saveTrigger, { peer, name: x.name, spec }), () => void refetch());
  };
  const confirm = (): void => {
    const d = deleting();
    setDeleting(undefined);
    if (!d) return;
    if (d.what === 'recipe') after(c.run(removeRecipe, { peer, name: d.name }), () => void again());
    else after(c.run(removeTrigger, { peer, name: d.name }), () => void refetch());
  };
  const recipeTags = (r: Recipe): string[] =>
    [
      fieldsOf(r)
        .map((f) => `{${f}}`)
        .join(' '),
      r.project ? place(r.project) : say('automations.form.anyProjectShort'),
      r.agent ? (harnesses().find((h) => h.id === r.agent)?.name ?? r.agent) : '',
    ].filter(Boolean);
  const triggerTags = (x: Trigger): string[] => {
    const spec = x.spec ?? {};
    const mode = typeof spec['mode'] === 'string' ? spec['mode'] : '';
    const agent = typeof spec['agent'] === 'string' ? spec['agent'] : '';
    const ago = x.lastRunAt ? say('automations.page.lastRun', { ago: timeLabel(x.lastRunAt, Date.now(), englishT) }) : say('automations.page.notRun');
    return [mode ? say(`automations.recipe.mode.${mode}`) : '', agent ? (harnesses().find((h) => h.id === agent)?.name ?? agent) : '', ago].filter(Boolean);
  };
  return (
    <AutomationsBody
      lede={[say('automations.page.lede1'), { code: '{name}' }, say('automations.page.lede2')]}
      tabs={[
        { value: 'recipes', label: say('automations.recipes'), count: (rs() ?? []).length },
        { value: 'triggers', label: say('automations.triggers'), count: (ts() ?? []).length },
      ]}
      tab={tab()}
      onTab={(v) => {
        setTab(v === 'triggers' ? 'triggers' : 'recipes');
        setEditing(undefined);
      }}
    >
      <Switch>
        <Match when={tab() === 'triggers' && editing()}>
          {(e) => (
            <TriggerForm
              client={c}
              recipes={rs() ?? []}
              here={parsePick(projectFilter())[0]}
              draft={e().draft}
              old={e().old}
              onDone={(saved) => {
                setEditing(undefined);
                if (saved) void refetch();
              }}
            />
          )}
        </Match>
        <Match when={tab() === 'triggers'}>
          <AutoList
            items={(ts() ?? []).map((x) => {
              const spec = x.spec ?? {};
              const fire = fireLabel(spec);
              return {
                id: x.name,
                icon: KINDS.find((k) => k.key === keyOfSpec(spec))?.icon ?? 'replay',
                name: x.name,
                line: [x.why || describeSpec(spec, { place, member, words: (cron) => cronWords(cron) })],
                tags: triggerTags(x),
                off: x.state === 'off',
                actions: (
                  <>
                    <Show when={fire}>
                      {(f) => (
                        <Button size="xs" variant="ghost" icon="replay" part="automations.trigger.fire" onClick={() => after(c.run(fireTrigger, { peer, name: x.name }), () => void refetch())}>
                          {say(f() === 'run' ? 'automations.page.runNow' : 'automations.page.checkNow')}
                        </Button>
                      )}
                    </Show>
                    <KitSwitch aria-label={say('automations.page.onOff')} checked={x.state !== 'off'} onChange={() => toggle(x)} />
                    <IconButton
                      icon="pen"
                      size="sm"
                      label={say('automations.page.edit')}
                      part="automations.trigger.edit"
                      disabled={!formOfSpec(x.name, spec)}
                      {...(!formOfSpec(x.name, spec) && { title: say('automations.page.formulaOnly') })}
                      onClick={() => setEditing({ old: { name: x.name, spec } })}
                    />
                    <IconButton icon="trash" size="sm" label={say('automations.page.delete')} part="automations.trigger.delete" onClick={() => setDeleting({ what: 'trigger', name: x.name })} />
                  </>
                ),
              };
            })}
            empty={say('automations.page.noTriggers')}
            add={{ label: say('automations.page.newTrigger'), onClick: () => setEditing({}) }}
            hint={[say('automations.page.triggersHint1'), { code: 'gh' }, say('automations.page.triggersHint2')]}
          />
        </Match>
        <Match when={tab() === 'recipes'}>
          <AutoList
            items={(rs() ?? []).map((r) => ({
              id: r.name,
              icon: 'bolt',
              name: r.name,
              line: [r.prompt.replace(/\s+/g, ' ').slice(0, 140)],
              tags: recipeTags(r),
              actions: (
                <>
                  <Button size="sm" variant="primary" part="automations.recipe.run" onClick={() => run(r)}>
                    {say('automations.page.run')}
                  </Button>
                  <IconButton
                    icon="clock"
                    size="sm"
                    label={say('automations.page.schedule')}
                    part="automations.recipe.schedule"
                    onClick={() => {
                      setTab('triggers');
                      setEditing({ draft: { key: 'schedule', name: r.name, recipe: r.name, ...(r.project && { project: r.project }), ...(r.mode && { mode: r.mode }) } });
                    }}
                  />
                  <IconButton icon="pen" size="sm" label={say('automations.page.edit')} part="automations.recipe.edit" onClick={() => edit(r)} />
                  <IconButton icon="trash" size="sm" label={say('automations.page.delete')} part="automations.recipe.delete" onClick={() => setDeleting({ what: 'recipe', name: r.name })} />
                </>
              ),
            }))}
            empty={say('automations.page.noRecipes')}
            add={{ label: say('automations.page.newRecipe'), onClick: () => openRecipe() }}
            hint={[say('automations.page.recipesHint1'), { code: 'wisp recipes' }, ', ', { code: 'wisp run <name> [values…]' }, '.']}
          />
        </Match>
      </Switch>
      {/* v1 ask(): Delete asks first, as every destructive step does (audit A #8: Remove went at once) */}
      <Dialog
        open={!!deleting()}
        onOpenChange={(o) => !o && setDeleting(undefined)}
        size="sm"
        role="alertdialog"
        part="automations.delete"
        title={say(deleting()?.what === 'recipe' ? 'automations.page.deleteRecipe' : 'automations.page.deleteTrigger', { name: deleting()?.name ?? '' })}
        description={say('automations.page.deleteWhy')}
        footer={
          <>
            <Button variant="secondary" size="sm" onClick={() => setDeleting(undefined)}>
              {say('automations.form.cancel')}
            </Button>
            <Button variant="danger" size="sm" part="automations.delete.ok" onClick={confirm}>
              {say(deleting()?.what === 'recipe' ? 'automations.page.deleteRecipe.ok' : 'automations.page.deleteTrigger.ok')}
            </Button>
          </>
        }
      />
    </AutomationsBody>
  );
}

function TriggerForm(props: {
  readonly client: PluginClient;
  readonly recipes: readonly Recipe[];
  readonly here?: string | undefined;
  /** a new trigger's start (a recipe's Schedule: its name, the recipe, its project and mode) */
  readonly draft?: Partial<TriggerFormState> | undefined;
  /** Edit: the trigger as saved; Save keeps what the form doesn't show, and its secret when none is typed */
  readonly old?: { readonly name: string; readonly spec: Readonly<Record<string, unknown>> } | undefined;
  readonly onDone: (saved: boolean) => void;
}): JSX.Element {
  const c = props.client;
  const shell = useShell();
  const say = t as (k: string, v?: Readonly<Record<string, string | number>>) => string;
  const projectRows = useSignal(c.entities.list<unknown>('project'));
  const projects = createMemo(() => projectList(projectRows(), undefined));
  const crew = useSignal(c.entities.list<{ id: string; name?: string; role?: string; body?: { name?: string; role?: string } }>('crew_member'));
  const harnesses = useSignal(shell.harnesses());
  const start = projects().find((p) => p.id === props.here)?.path ?? projects()[0]?.path ?? '';
  const old = props.old;
  const [f, setF] = createSignal<TriggerFormState>((old && formOfSpec(old.name, old.spec)) || { ...emptyForm(start), ...props.draft });
  const [busy, setBusy] = createSignal(false);
  const [problem, setProblem] = createSignal('');
  const set = (patch: Partial<TriggerFormState>): void => void setF({ ...f(), ...patch });
  // the project the list shows, else the first (v1: the launcher's), once the projects have arrived; never over a pick
  let picked = false;
  createEffect(() => {
    const first = projects().find((p) => p.id === props.here)?.path ?? projects()[0]?.path;
    if (!picked && !old && first && !f().project) setF({ ...f(), project: first });
  });
  const [picking, setPicking] = createSignal(false);
  const kind = (): string => kindOf(f().key).kind;
  const hook = (): boolean => HOOK_KINDS.has(kind());
  const members = (): { id: string; label: string }[] =>
    (crew() ?? []).map((m) => {
      const name = m.name ?? m.body?.name ?? m.id;
      const role = m.role ?? m.body?.role;
      return { id: m.id, label: `${name}${role ? ` · ${role}` : ''} (crew)` };
    });
  const save = (): void => {
    const out = old ? specOfEdit(f(), old) : specOfForm(f());
    if ('unsupported' in out) return void setProblem(out.unsupported);
    if ('missing' in out) return void setProblem(say(`automations.form.missing.${out.missing}`));
    setProblem('');
    setBusy(true);
    c.run(saveTrigger, { peer: c.home, name: out.name, spec: out.spec }).then(
      () => props.onDone(true),
      () => setBusy(false),
    );
  };
  const anyProject = (): string => (TASK_KINDS.has(kind()) ? say(f().crew ? 'automations.form.theirProject' : 'automations.form.anyProject') : say('automations.form.ownFolder'));
  // the New button that opened the form is gone: focus stays in the form on the picked kind (v1 focused no field)
  let root: HTMLDivElement | undefined;
  onMount(() => root?.querySelector<HTMLElement>('[aria-pressed="true"]')?.focus({ preventScroll: true }));
  return (
    <div data-part="automations.form" ref={root}>
      <TriggerKinds kinds={KINDS.map((k) => ({ key: k.key, label: k.label, hint: k.hint, icon: k.icon }))} value={f().key} onPick={(k) => setF(pick(f(), k))} />
      <AutoRow label={say('automations.form.name')}>
        <Input value={f().name} onInput={(v) => set({ name: v })} maxLength={60} spellcheck={false} disabled={!!old} aria-label={say('automations.form.name')} />
      </AutoRow>
      <Show when={members().length > 0}>
        <AutoRow label={say('automations.form.sendTo')}>
          <Select
            aria-label={say('automations.form.sendTo')}
            value={f().crew}
            onChange={(v) => set({ crew: v })}
            items={[{ value: '', label: say('automations.form.newTask') }, ...members().map((m) => ({ value: m.id, label: m.label }))]}
          />
        </AutoRow>
      </Show>
      <AutoRow label={say('automations.form.project')}>
        <Select
          aria-label={say('automations.form.project')}
          value={f().project}
          onChange={(v) => ((picked = true), set({ project: v }))}
          items={[...(TASK_KINDS.has(kind()) || members().length ? [{ value: '', label: anyProject() }] : []), ...projects().map((p) => ({ value: p.path, label: p.name }))]}
        />
      </AutoRow>
      <Show when={kind() === 'schedule'}>
        <AutoRow label={say('automations.form.when')}>
          <Input
            value={f().when}
            onInput={(v) => set({ when: v })}
            spellcheck={false}
            autocomplete="off"
            placeholder={say('automations.form.whenPlaceholder')}
            aria-label={say('automations.form.when')}
          />
        </AutoRow>
        <AutoPresets items={SCHEDULE_PRESETS} onPick={(w) => set({ when: w })} read={readBack(f().when, (cron) => cronWords(cron))} />
      </Show>
      <Show when={kind() === 'ci'}>
        <AutoRow label={say('automations.form.branch')}>
          <Input mono value={f().branch} onInput={(v) => set({ branch: v })} spellcheck={false} placeholder={say('automations.form.branchPlaceholder')} aria-label={say('automations.form.branch')} />
        </AutoRow>
      </Show>
      <Show when={kind() === 'issues'}>
        <AutoRow label={say('automations.form.label')}>
          <Input value={f().label} onInput={(v) => set({ label: v })} spellcheck={false} placeholder="wisp" aria-label={say('automations.form.label')} />
        </AutoRow>
      </Show>
      <Show when={hook()}>
        <Show when={kind() !== 'slack'}>
          <AutoRow label={say('automations.form.url')}>
            <small>{say('automations.form.urlLater')}</small>
          </AutoRow>
        </Show>
        <AutoNote tone="muted">{say(`automations.form.how.${kind()}`)}</AutoNote>
        <Show when={kind() !== 'slack'}>
          <AutoRow label={say(kind() === 'webhook' ? 'automations.form.secret' : 'automations.form.signingSecret')}>
            <Input
              mono
              type="password"
              value={f().secret}
              onInput={(v) => set({ secret: v })}
              autocomplete="new-password"
              spellcheck={false}
              placeholder={old && typeof old.spec['secret'] === 'string' && old.spec['secret'] ? say('automations.form.secretKept') : say(`automations.form.secretPlaceholder.${kind()}`)}
              aria-label={say('automations.form.secret')}
            />
          </AutoRow>
        </Show>
        <Show when={kind() === 'webhook'}>
          <AutoRow label={say('automations.form.header')}>
            <Input mono value={f().header} onInput={(v) => set({ header: v })} spellcheck={false} placeholder="X-Hub-Signature-256" aria-label={say('automations.form.header')} />
          </AutoRow>
        </Show>
        <Show when={kind() === 'slack'}>
          <AutoRow label={say('automations.form.people')}>
            <Input mono value={f().users} onInput={(v) => set({ users: v })} spellcheck={false} autocomplete="off" placeholder="U0123ABCDE, U0456FGHIJ" aria-label={say('automations.form.people')} />
          </AutoRow>
          <AutoNote tone="muted">{say('automations.form.peopleHow')}</AutoNote>
          <AutoRow label={say('automations.form.keyword')}>
            <Input value={f().keyword} onInput={(v) => set({ keyword: v })} spellcheck={false} placeholder={say('automations.form.keywordPlaceholder')} aria-label={say('automations.form.keyword')} />
          </AutoRow>
        </Show>
        <Show when={kind() === 'linear'}>
          <AutoRow label={say('automations.form.label')}>
            <Input value={f().label} onInput={(v) => set({ label: v })} spellcheck={false} placeholder={say('automations.form.linearPlaceholder')} aria-label={say('automations.form.label')} />
          </AutoRow>
        </Show>
      </Show>
      <Show when={POLLED.has(kind())}>
        <AutoRow label={say('automations.form.every')}>
          <Select
            aria-label={say('automations.form.every')}
            value={String(f().every)}
            onChange={(v) => set({ every: Number(v) })}
            items={EVERY.map((m) => ({ value: String(m), label: say('automations.form.minutes', { n: m }) }))}
          />
        </AutoRow>
      </Show>
      <AutoRow label={say('automations.form.run')}>
        <Select
          aria-label={say('automations.form.run')}
          value={f().recipe}
          onChange={(v) => set({ recipe: v })}
          items={[{ value: '', label: say('automations.form.thisPrompt') }, ...props.recipes.map((r) => ({ value: r.name, label: say('automations.form.recipe', { name: r.name }) }))]}
        />
      </AutoRow>
      <Show when={!f().recipe}>
        <AutoRow stack label={say('automations.form.prompt')}>
          <TextArea mono value={f().prompt} onInput={(v) => set({ prompt: v })} minRows={6} maxRows={12} spellcheck={false} aria-label={say('automations.form.prompt')} />
        </AutoRow>
      </Show>
      <Show when={!f().crew} fallback={<AutoNote tone="muted">{say('automations.form.crewRuns')}</AutoNote>}>
        <AutoRow label={say('automations.form.mode')}>
          <Select
            aria-label={say('automations.form.mode')}
            value={f().mode}
            onChange={(v) => set({ mode: v })}
            items={['plan', 'edits', 'auto'].map((m) => ({ value: m, label: say(`automations.recipe.mode.${m}`) }))}
          />
        </AutoRow>
        <AutoRow label={say('automations.form.harness')}>
          {/* v1 runnerField: the runner button opens the harness · model picker; Use launcher default clears it */}
          <AutoRunner
            harness={f().agent ? (harnesses().find((h) => h.id === f().agent)?.name ?? f().agent) : say('automations.form.launcherDefault')}
            model={f().agent ? f().model || say('automations.form.defaultModel') : undefined}
            logo={harnesses().find((h) => h.id === f().agent)?.logo}
            title={say('automations.form.harnessTitle')}
            onClick={() => setPicking(!picking())}
            reset={f().agent ? { label: say('automations.form.useDefault'), onClick: () => set({ agent: '', model: '' }) } : undefined}
          />
          <RunnerPicker
            open={picking()}
            onOpenChange={setPicking}
            anchor={() => document.querySelector('[data-part="automations.runner"] [data-kit="au-runner-btn"]')}
            peer={c.home}
            efforts={false}
            value={{ agent: f().agent, model: f().model, effort: '' }}
            onChange={(r) => set({ agent: r.agent, model: r.model })}
          />
        </AutoRow>
      </Show>
      <Show when={warning(f())}>{(w) => <AutoNote tone="warn">{w()}</AutoNote>}</Show>
      <Show when={problem()}>{(p) => <AutoNote tone="warn">{p()}</AutoNote>}</Show>
      <AutoFoot cancel={say('automations.form.cancel')} ok={say(old ? 'automations.form.save' : 'automations.form.create')} busy={busy()} onCancel={() => props.onDone(false)} onOk={save} />
    </div>
  );
}
