// What the harness picker reads, kept per peer between opens (perf-ui-r2 #3): each open sent 10 requests
// (`runtime.agents`, `accounts.list`, the harnesses topic, and one `budget.view` per harness) and applied their seven
// usage answers one at a time, re-rendering the rows on each. Now a hover on the pill warms it, an open within
// `FRESH_MS` reuses what's here (no request at all), and the usage answers land in one update. Plain (no Solid) so it
// is tested on its own; the picker turns it into signals.

import { defineCommand, type CommandDef, type PeerId, type Scope } from '@wisp/client';
import type { Topic } from '@wisp/protocol';
import { fullest, type AccountOffer, type AgentOffer, type PlanWindow } from './runner.ts';

/** How long an answer is reused: the picker's sub-lines (`5h 42% used`) and account list move slowly. */
export const FRESH_MS = 30_000;
/** An open of the picker asks each harness's usage again unless it was read this recently (a hover on the pill warmed
 *  it): usage refreshes on each open, not once per 30 s (harness audit r2). */
export const OPEN_MS = 3_000;

export const agentsOf = defineCommand<{ peer: string }, AgentOffer[]>({ id: 'runtime.agents', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'the harness list' });

export const plansOf = defineCommand<{ peer: string; agent: string }, { windows: PlanWindow[] }[]>({
  id: 'budget.view',
  owner: (a) => a.peer,
  wire: (a) => ({ agent: a.agent }),
  optimistic: 'none',
  pending: 'the plan windows',
});

/** A harness's accounts (wisp.accounts); refused or absent: none, and the picker shows no account choice. */
export const accountsOf = defineCommand<{ peer: string; agent: string }, { accounts: AccountOffer[] }>({
  id: 'accounts.list',
  owner: (a) => a.peer,
  wire: (a) => ({ agent: a.agent }),
  optimistic: 'none',
  pending: 'the account list',
});

type Run = <A, R>(cmd: CommandDef<A, R>, args: A) => Promise<R>;

interface Kept<T> {
  readonly at: number;
  readonly p: Promise<T>;
  value?: T;
}

/** One peer's picker data: asked once per `FRESH_MS`, an answer in flight shared by whoever asks meanwhile. */
export interface PickerData {
  /** `runtime.agents` */
  agents(): Promise<AgentOffer[]>;
  /** the agents' answer if it is here and fresh (an open draws with it at once) */
  agentsNow(): AgentOffer[] | undefined;
  /** each listed harness's fullest plan window, asked together and given back as one map; `maxAge` asks again for an
   *  answer older than it (the picker's open: `OPEN_MS`, so each open shows the plan as it is now) */
  usage(agents: readonly string[], maxAge?: number): Promise<ReadonlyMap<string, PlanWindow | null | undefined>>;
  /** what `usage` would answer from what's here and fresh, without asking */
  usageNow(agents: readonly string[]): ReadonlyMap<string, PlanWindow | null | undefined>;
  accounts(agent: string): Promise<AccountOffer[]>;
  /** a hover on the pill: the harness list, then every set-up harness's usage, and `agent`'s accounts */
  warm(agent?: string): void;
}

export function pickerData(run: Run, peer: string, now: () => number): PickerData {
  let agents: Kept<AgentOffer[]> | undefined;
  const plans = new Map<string, Kept<PlanWindow | null | undefined>>();
  const accounts = new Map<string, Kept<AccountOffer[]>>();
  const fresh = <T>(k: Kept<T> | undefined): k is Kept<T> => k !== undefined && now() - k.at < FRESH_MS;
  const keep = <T>(p: Promise<T>): Kept<T> => {
    const k: Kept<T> = { at: now(), p };
    void p.then((v) => (k.value = v));
    return k;
  };
  const plan = (agent: string, maxAge = FRESH_MS): Kept<PlanWindow | null | undefined> => {
    let k = plans.get(agent);
    if (!fresh(k) || now() - k.at >= maxAge) {
      // a refusal or a dropped link answers "nothing" now, and isn't kept: the next open asks again
      // read with no window: null ("limits not reported"), not undefined (not read)
      k = keep(run(plansOf, { peer, agent }).then((accs) => fullest(accs) ?? null, () => (plans.delete(agent), undefined)));
      plans.set(agent, k);
    }
    return k;
  };
  const d: PickerData = {
    agents() {
      if (!fresh(agents))
        agents = keep(
          run(agentsOf, { peer }).catch(() => {
            agents = undefined;
            return [] as AgentOffer[];
          }),
        );
      return agents.p;
    },
    agentsNow: () => (fresh(agents) ? agents.value : undefined),
    async usage(ids, maxAge) {
      const ks = ids.map((a) => [a, plan(a, maxAge)] as const);
      const vs = await Promise.all(ks.map(([, k]) => k.p));
      return new Map(ks.map(([a], i) => [a, vs[i]]));
    },
    usageNow(ids) {
      const out = new Map<string, PlanWindow | null | undefined>();
      for (const a of ids) {
        const k = plans.get(a);
        if (fresh(k) && 'value' in k) out.set(a, k.value);
      }
      return out;
    },
    accounts(agent) {
      let k = accounts.get(agent);
      if (!fresh(k)) {
        k = keep(run(accountsOf, { peer, agent }).then((r) => r.accounts, () => (accounts.delete(agent), [] as AccountOffer[])));
        accounts.set(agent, k);
      }
      return k.p;
    },
    warm(agent) {
      void d.agents().then((list) => d.usage(list.filter((a) => a.available && !a.checking).map((a) => a.id)));
      if (agent && agent !== 'auto') void d.accounts(agent);
    },
  };
  return d;
}

// one per client and peer: the shell's whole run (a client is the app's, so this lives as long as it does)
const perClient = new WeakMap<object, Map<string, PickerData>>();

/** The picker data of `peer` on this client, made on first use. */
export function pickerDataOf(client: { readonly commands: { run: Run }; readonly scope: { readonly timers: { now(): number } } }, peer: string): PickerData {
  let m = perClient.get(client);
  if (!m) perClient.set(client, (m = new Map()));
  let d = m.get(peer);
  if (!d) {
    d = pickerData((cmd, args) => client.commands.run(cmd, args), peer, () => client.scope.timers.now());
    m.set(peer, d);
  }
  return d;
}

/** What `holdHarnesses` needs of the client: its peers' topics and its scope's timers. */
export interface Watcher {
  readonly peers: { watch(peer: PeerId, topic: Topic, owner: Scope): void };
  readonly scope: Scope;
}

const held = new WeakMap<object, Map<string, { n: number; stop: () => void; linger?: (() => void) | undefined }>>();

/** The harnesses topic watched while a picker shows, and `FRESH_MS` after the last one closes: an open soon after
 *  finds it still there (no new subscribe, no snapshot to apply). Returns the release. */
export function holdHarnesses(client: Watcher, peer: string, topic: Topic): () => void {
  let m = held.get(client);
  if (!m) held.set(client, (m = new Map()));
  const map = m;
  let h = map.get(peer);
  if (!h) {
    const scope = client.scope.child('runner-harnesses');
    client.peers.watch(peer as PeerId, topic, scope);
    h = { n: 0, stop: () => scope.dispose() };
    map.set(peer, h);
  }
  const it = h;
  it.linger?.();
  it.linger = undefined;
  it.n++;
  let done = false;
  return () => {
    if (done) return;
    done = true;
    if (--it.n > 0) return;
    // a timer on the client's scope, not the watch's own (disposing that would cancel it)
    it.linger = client.scope.timeout(FRESH_MS, () => {
      if (it.n > 0) return;
      it.stop();
      map.delete(peer);
    });
  };
}
