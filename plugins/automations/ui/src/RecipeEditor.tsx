// A recipe from a task (v1 automations.js recipeForm, opened by Save as recipe… in a task's menu): its name, the
// prompt (`{version}` becomes a field when it's run), the project (any, picked when run), the mode (the launcher's
// default) and the task's harness, model and effort as they were. Create recipe saves it (`automations.task.set`) and
// opens Settings → Automations on it; Cancel keeps nothing.
import { createMemo, createSignal, Show, type JSX } from 'solid-js';
import { defineCommand, type PeerId, type PluginClient } from '@wisp/client';
import { projectList, useRunWords, useShell } from '@wisp/shell';
import { Button, Dialog, Field, Input, Inline, Select, Stack, Text, TextArea, useSignal } from '@wisp/ui';
import { t } from './i18n.ts';
import { recipeBody, recipeName, recipeParams, type RecipeDraft } from './recipe.ts';
import './i18n/lazy.ts';

const save = defineCommand<{ peer: PeerId; draft: RecipeDraft; base?: Readonly<Record<string, unknown>> | undefined }, unknown>({
  id: 'automations.task.set',
  owner: (a) => a.peer,
  wire: (a) => recipeBody(a.draft, a.base),
  optimistic: 'none',
  pending: 'Create recipe',
});
/** Edit that renamed it: the old name goes once the new one is saved (v1 PATCHed one row; v2 keys a task by name) */
const removeOld = defineCommand<{ peer: PeerId; name: string }, unknown>({ id: 'automations.task.remove', owner: (a) => a.peer, wire: (a) => ({ name: a.name }), optimistic: 'none', pending: 'Save' });
/** the recipe as saved, for an edit: what the editor doesn't show is kept */
const saved = defineCommand<{ peer: PeerId }, readonly Record<string, unknown>[]>({ id: 'automations.tasks', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'Save' });

/** the task's first turn (v1 saveAsRecipe: `s.turns[0].prompt`) */
const firstTurn = defineCommand<{ key: string; id: string }, readonly { prompt?: string }[]>({
  id: 'runtime.turns',
  owner: (a) => a.key,
  wire: (a) => ({ id: a.id, before: 1, limit: 1 }),
  optimistic: 'none',
  pending: 'the task’s first prompt',
});

export const RECIPE_VIEW = 'automations.recipe';

export default function RecipeEditor(props: { readonly client: PluginClient; readonly params: RecipeDraft }): JSX.Element {
  const c = props.client;
  const say = t as (k: string, v?: Readonly<Record<string, string | number>>) => string;
  // the run in the picker's words (QA r15 #3: it printed "claude · opus · high")
  const runWords = useRunWords(useShell());
  const [name, setName] = createSignal(recipeName(props.params.name));
  const editing = props.params.editing;
  const [prompt, setPrompt] = createSignal(props.params.prompt);
  const from = props.params.session;
  if (from && !props.params.prompt)
    void c
      .run(firstTurn, { key: from, id: from.slice(from.indexOf(':') + 1) })
      .then((turns) => !prompt() && setPrompt(turns[0]?.prompt ?? ''))
      .catch(() => undefined);
  const [project, setProject] = createSignal(props.params.project ?? '');
  const [mode, setMode] = createSignal(props.params.mode ?? '');
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal('');
  const rows = useSignal(c.entities.list<unknown>('project'));
  const projects = createMemo(() => projectList(rows(), undefined));
  const params = createMemo(() => recipeParams(prompt()));
  const close = (): void => c.navigate({ t: 'closeOverlay', id: RECIPE_VIEW as never });
  const create = (): void => {
    if (!recipeName(name()) || !prompt().trim()) {
      setError(say('automations.recipe.need'));
      return;
    }
    setBusy(true);
    setError('');
    const as = recipeName(name());
    const draft = { ...props.params, name: as, prompt: prompt(), project: project(), mode: mode() };
    (editing ? c.run(saved, { peer: c.home }).then((all) => all.find((r) => r['name'] === editing)) : Promise.resolve(undefined))
      .then((base) => c.run(save, { peer: c.home, draft, base }))
      .then(() => (editing && editing !== as ? c.run(removeOld, { peer: c.home, name: editing }) : undefined))
      .then(
        () => {
          close();
          // Edit came from the Automations list, which reads its lists again; Save as recipe… opens it on the recipe
          if (editing) return;
          c.navigate({ t: 'openOverlay', overlay: { kind: 'viewer', id: 'shell-settings.sheet' as never, params: { pane: 'automations' } } });
        },
        (e: unknown) => {
          setBusy(false);
          setError(e instanceof Error ? e.message : String(e));
        },
      );
  };
  return (
    <Dialog
      part="automations.recipe"
      title={say(editing ? 'automations.recipe.edit' : 'automations.recipe.title')}
      open
      onOpenChange={(o) => !o && close()}
      closeOnOutside={false}
      footer={
        <Inline gap="2" justify="end">
          <Button variant="ghost" size="sm" onClick={close}>
            {say('automations.recipe.cancel')}
          </Button>
          <Button variant="primary" size="sm" loading={busy()} onClick={create}>
            {say(editing ? 'automations.recipe.save' : 'automations.recipe.create')}
          </Button>
        </Inline>
      }
    >
      <Stack gap="4">
        <Field label={say('automations.recipe.name')} {...(recipeName(name()) && recipeName(name()) !== name().trim() && { hint: say('automations.recipe.savedAs', { name: recipeName(name()) }) })}>
          <Input value={name()} onInput={setName} maxLength={60} spellcheck={false} placeholder={say('automations.recipe.namePlaceholder')} autofocus />
        </Field>
        <Field
          label={say('automations.recipe.prompt')}
          hint={
            params().length
              ? say('automations.recipe.fields', {
                  fields: params()
                    .map((p) => `{${p}}`)
                    .join(' '),
                })
              : say('automations.recipe.noFields')
          }
        >
          <TextArea value={prompt()} onInput={setPrompt} minRows={5} maxRows={12} spellcheck={false} mono />
        </Field>
        <Field label={say('automations.recipe.project')}>
          <Select
            aria-label={say('automations.recipe.project')}
            value={project()}
            onChange={setProject}
            items={[{ value: '', label: say('automations.recipe.anyProject') }, ...projects().map((p) => ({ value: p.path, label: p.name }))]}
          />
        </Field>
        <Field label={say('automations.recipe.mode')}>
          <Select
            aria-label={say('automations.recipe.mode')}
            value={mode()}
            onChange={setMode}
            items={['', 'plan', 'edits', 'auto'].map((m) => ({ value: m, label: say(`automations.recipe.mode.${m || 'default'}`) }))}
          />
        </Field>
        <Show when={props.params.agent}>
          {(a) => (
            <Text size="sm" tone="muted">
              {say('automations.recipe.runsOn', { runner: runWords({ agent: a(), model: props.params.model, effort: props.params.effort, cwd: props.params.project || undefined }) })}
            </Text>
          )}
        </Show>
        <Show when={error()}>
          <Text size="sm" tone="bad">
            {error()}
          </Text>
        </Show>
      </Stack>
    </Dialog>
  );
}
