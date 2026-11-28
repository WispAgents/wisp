// A webhook trigger's grid URLs on the mock (ADR-0147): `grid-client.hooks`, `.hooksCreate` and `.hooksRevoke`
// answered as wisp answers them (plugins/grid-client/api/bindings/grid-client/{TriggerHook,NewHook}.ts), and the
// triggers the form saves (`resources.apply`, kind trigger) listed back by `automations.trigger.ls`, so a saved
// webhook trigger's form shows its URLs. A made URL is answered once and kept nowhere here either: the list holds its
// last four characters only.

type Fn = (args: unknown) => { ops: never[]; result?: unknown };
type Args = Readonly<Record<string, unknown>>;

interface Hook {
  id: string;
  trigger: string;
  computer: string;
  hint: string;
  createdAt: number;
  lastUsedAt: number | null;
}

const MIN = 60_000;
const COMPUTER = 'studio';
const ADDRESS = 'https://studio--ada.trywisp.dev';
const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
const token = (): string => `wgh_${Array.from({ length: 43 }, () => B64[Math.floor(Math.random() * 64)]).join('')}`;
const hex = (): string => Array.from({ length: 12 }, () => '0123456789abcdef'[Math.floor(Math.random() * 16)]).join('');
const refuse = (code: string, message: string): never => {
  throw { code, message };
};
const obj = (v: unknown): Args => (v && typeof v === 'object' && !Array.isArray(v) ? (v as Args) : {});

let hooks: Hook[] | undefined;
const live = (): Hook[] =>
  (hooks ??= [{ id: 'a1b2c3d4e5f6', trigger: 'deploy', computer: COMPUTER, hint: 'Zq9x', createdAt: Date.now() - 2 * 24 * 60 * MIN, lastUsedAt: Date.now() - 7 * MIN }]);
const triggers = new Map<string, Args>();

/** The mock starts over (POST /__mock/reset): no saved triggers, the one seeded URL again. */
export function resetHooks(): void {
  hooks = undefined;
  triggers.clear();
}

/** `resources.apply`'s triggers, kept for `automations.trigger.ls`. */
export function keepTriggers(resources: readonly Args[]): void {
  for (const r of resources.filter((x) => x['kind'] === 'trigger')) triggers.set(String(r['name'] ?? ''), obj(r['spec']));
}

export const HOOK_COMMANDS: Record<string, Fn> = {
  'automations.trigger.ls': () => ({
    ops: [],
    result: [...triggers].map(([name, spec]) => ({ name, spec, from: 'trigger', state: spec['enabled'] === false ? 'off' : 'on', why: '', runs: 0, queued: 0, awaiting: 0 })),
  }),
  'grid-client.hooks': (a) => {
    const t = String(obj(a)['trigger'] ?? '').trim();
    return { ops: [], result: live().filter((h) => !t || h.trigger === t) };
  },
  'grid-client.hooksCreate': (a) => {
    const trigger = String(obj(a)['trigger'] ?? '').trim();
    if (!trigger) refuse('invalid', 'trigger: name the webhook trigger this URL starts');
    const tok = token();
    const h: Hook = { id: hex(), trigger, computer: COMPUTER, hint: tok.slice(-4), createdAt: Date.now(), lastUsedAt: null };
    hooks = [h, ...live()];
    return { ops: [], result: { ...h, url: `${ADDRESS}/_hook/${tok}` } };
  },
  'grid-client.hooksRevoke': (a) => {
    const id = String(obj(a)['id'] ?? '');
    if (!live().some((h) => h.id === id)) refuse('not_found', 'No such URL.');
    hooks = live().filter((h) => h.id !== id);
    return { ops: [], result: { ok: true } };
  },
};
