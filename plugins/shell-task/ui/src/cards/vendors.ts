// Whose model a harness runs (v1 web/js/vendors.js), DOM-free: Anthropic, OpenAI, Google, xAI… A refusal is the
// model's answer, not the harness's: Copilot, opencode, Cursor and Antigravity can all run Claude or GPT models, so
// handing a Claude refusal to Copilot on its default could ask Anthropic's model again. `handoffTargets` only offers
// a harness whose model is known to come from another vendor, and pins that model on the hand-off.

const VENDORS: readonly (readonly [string, RegExp])[] = [
  ['anthropic', /claude|anthropic|(?:^|[/:\s_-])(?:opus|sonnet|haiku|fable)(?:$|[/:\s._-]|\d)/i],
  ['openai', /openai|chatgpt|gpt|codex|(?:^|[/:\s_-])o\d(?:$|[/:\s._-])/i],
  ['google', /gemini|gemma|google|vertex/i],
  ['xai', /grok|xai|x-ai/i],
  ['deepseek', /deepseek/i],
  ['alibaba', /qwen/i],
  ['moonshot', /kimi|moonshot/i],
  ['zhipu', /glm|zhipu|z-ai/i],
  ['mistral', /mistral|codestral|devstral/i],
  ['meta', /llama/i],
  ['cursor', /(?:^|[/:\s_-])composer/i],
];

/** What a harness runs when it isn't given a model: only the single-vendor ones are sure. */
const HOME: Readonly<Record<string, string>> = { claude: 'anthropic', codex: 'openai', grok: 'xai' };

export function vendorOf(model: string | undefined, agent: string): string {
  const m = String(model ?? '').trim();
  if (m && !/^(?:auto|default)$/i.test(m)) {
    for (const [v, re] of VENDORS) if (re.test(m)) return v;
  }
  return HOME[agent] ?? '';
}

export interface AgentLike {
  readonly id: string;
  readonly name?: string;
  readonly available?: boolean;
  readonly models?: readonly { readonly id: string }[];
}
export interface Target {
  readonly agent: string;
  readonly model: string;
  readonly vendor: string;
  /** the limit card: what its plan has left, 0–1 (wisp.budget), when known */
  readonly left?: number;
}

/** The harnesses a refused turn may go to, each with the model to pin (v1 handoffTargets). A harness qualifies
 * with its configured model when that model's vendor is known and different, else with the first model it lists
 * from another vendor. Unknown vendors never qualify: they might be the same one. */
export function handoffTargets(o: { from: string; fromModel?: string; vendor?: string; agents: readonly AgentLike[]; models?: Readonly<Record<string, string>>; max?: number }): Target[] {
  // the runtime's word on whose model refused, else read from the model; unknown: every other harness (backend round 6)
  const refused = o.vendor ?? vendorOf(o.fromModel, o.from);
  const ok = (v: string): boolean => !!v && v !== refused;
  const out: Target[] = [];
  if (!refused) {
    for (const a of o.agents) {
      if (a.id === o.from || a.id === 'wisp' || a.id === 'auto' || !a.available) continue;
      out.push({ agent: a.id, model: String(o.models?.[a.id] ?? ''), vendor: vendorOf(o.models?.[a.id], a.id) });
      if (out.length >= (o.max ?? 3)) break;
    }
    return out;
  }
  for (const a of o.agents) {
    if (a.id === o.from || a.id === 'wisp' || a.id === 'auto' || !a.available) continue;
    const set = String(o.models?.[a.id] ?? '');
    const own = vendorOf(set, a.id);
    if (ok(own)) out.push({ agent: a.id, model: set, vendor: own });
    else {
      const pick = (a.models ?? []).find((m) => m.id && ok(vendorOf(m.id, a.id)));
      if (pick) out.push({ agent: a.id, model: pick.id, vendor: vendorOf(pick.id, a.id) });
    }
    if (out.length >= (o.max ?? 3)) break;
  }
  return out;
}

/** One plan window as wisp.budget's `budget.view` reads it (budget-api WindowView): `used` 0–1. */
export interface PlanWindowLike {
  readonly used: number;
  readonly resetsAt?: number | null;
}
/** One account of a harness (budget-api AccountView): its windows. */
export interface PlanLike {
  readonly account: string;
  readonly windows: readonly PlanWindowLike[];
}

/** What a harness has left now, 0–1: its roomiest account's tightest window still in its period. undefined: wisp.budget
 * knows no window for it (not read yet, or a CLI that reports none). */
export function headroom(plans: readonly PlanLike[] | undefined, now: number): number | undefined {
  let best: number | undefined;
  for (const a of plans ?? []) {
    const live = a.windows.filter((w) => (w.resetsAt ?? 0) > now);
    if (!live.length) continue;
    const left = Math.max(0, Math.min(...live.map((w) => 1 - w.used)));
    best = best === undefined ? left : Math.max(best, left);
  }
  return best;
}

/** A harness at or past this share left is used up: handing a limited task to it would only hit its limit. */
const SPENT = 0.005;

/** The usage-limit card's other harness (v1 limitCard, check-in 50): every available one but this, ranked by what
 * its plan has left (wisp.budget's windows): the most room first, then the ones wisp knows nothing about (in the
 * order set up), never one that's used up itself. `left` rides along for the button's tip. */
export function limitTargets(from: string, agents: readonly AgentLike[], plans: Readonly<Record<string, readonly PlanLike[]>> = {}, now = 0): Target[] {
  const ranked = agents
    .filter((x) => x.id !== from && x.id !== 'wisp' && x.id !== 'auto' && x.available)
    .map((a, i) => ({ a, i, left: headroom(plans[a.id], now) }))
    .filter((x) => x.left === undefined || x.left > SPENT);
  ranked.sort((p, q) => (q.left ?? -1) - (p.left ?? -1) || p.i - q.i);
  return ranked.map(({ a, left }) => ({ agent: a.id, model: '', vendor: vendorOf('', a.id), ...(left !== undefined && { left }) }));
}

/** The usage-limit card's one other harness: the best of `limitTargets`. */
export function limitTarget(from: string, agents: readonly AgentLike[], plans?: Readonly<Record<string, readonly PlanLike[]>>, now?: number): Target | undefined {
  return limitTargets(from, agents, plans, now)[0];
}
