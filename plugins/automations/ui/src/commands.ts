// Settings → Automations' reads and writes: the saved prompts (recipes, `automations.tasks`) and the
// triggers that launch them (`automations.trigger.ls`); run a recipe now, remove one, fire a trigger now.
import { defineCommand, type PeerId } from '@wisp/client';

export interface Recipe {
  readonly name: string;
  readonly prompt: string;
  readonly steps?: readonly string[];
  readonly project?: string;
  readonly agent?: string;
  readonly model?: string;
  readonly effort?: string;
  readonly mode?: string;
}

export interface Trigger {
  readonly name: string;
  /** where it was declared (a formula, Settings) */
  readonly from: string;
  /** on | off | held | failing … */
  readonly state: string;
  readonly why?: string;
  readonly lastRunAt?: number | null;
  readonly runs: number;
  /** its declaration (`trigger` resource spec): Edit opens the form on it, the switch turns `enabled` off and on */
  readonly spec?: Readonly<Record<string, unknown>>;
}

export const recipes = defineCommand<{ peer: PeerId }, Recipe[]>({ id: 'automations.tasks', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'the recipes list' });
export const triggers = defineCommand<{ peer: PeerId }, Trigger[]>({ id: 'automations.trigger.ls', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'the triggers list' });
export const runRecipe = defineCommand<{ peer: PeerId; name: string }>({
  id: 'automations.task.run',
  owner: (a) => a.peer,
  wire: (a) => ({ name: a.name }),
  optimistic: 'none',
  pending: 'the recipe’s Run',
});
export const removeRecipe = defineCommand<{ peer: PeerId; name: string }>({
  id: 'automations.task.remove',
  owner: (a) => a.peer,
  wire: (a) => ({ name: a.name }),
  optimistic: 'none',
  pending: 'the recipe’s Remove',
});
export const fireTrigger = defineCommand<{ peer: PeerId; name: string }>({
  id: 'automations.trigger.fire',
  owner: (a) => a.peer,
  wire: (a) => ({ name: a.name }),
  optimistic: 'none',
  pending: 'the trigger’s Run now',
});

/** One event's dry run (`automations.trigger.test`): what the trigger would have done with it. */
export interface Tried {
  key: string;
  summary: string;
  result: string;
  why: string;
  prompt: string;
}
/** Would it have fired: the trigger on the last five events of its source, starting nothing. */
export const testTrigger = defineCommand<{ peer: PeerId; name: string }, Tried[]>({
  id: 'automations.trigger.test',
  owner: (a) => a.peer,
  wire: (a) => ({ name: a.name, last: 5 }),
  optimistic: 'none',
  pending: 'the trigger’s Test',
});

/** Save a trigger (Create, Edit, the on/off switch): its whole declaration, as `wisp apply` writes it (ADR-0043). */
export const saveTrigger = defineCommand<{ peer: PeerId; name: string; spec: Readonly<Record<string, unknown>> }>({
  id: 'resources.apply',
  owner: (a) => a.peer,
  wire: (a) => ({ resources: [{ kind: 'trigger', name: a.name, spec: a.spec }] }),
  optimistic: 'none',
  pending: 'the trigger',
});
/** Delete a trigger (v1's trash button, after its confirm): `wisp destroy trigger.<name>`. */
export const removeTrigger = defineCommand<{ peer: PeerId; name: string }>({
  id: 'resources.apply',
  owner: (a) => a.peer,
  wire: (a) => ({ resources: [], destroy: [`trigger.${a.name}`] }),
  optimistic: 'none',
  pending: 'the trigger’s Delete',
});
