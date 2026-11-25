// What a harness's Default runs, by name, one answer for the whole client (owner 2026-11-18: one name everywhere):
// Home's chip and picker, the task's facts and reply chip, Compare, the sidebar card and the list row all read this
// store. runtime.defaultModel is asked once per computer, harness and folder (the harness reads its config there, and
// its plan: never a paid call), and only while that computer is online: an asleep or away one isn't woken, and no
// command waits to send for it. A failed ask waits a minute before the next, so a list's re-renders don't repeat it.
// Only the folder in focus asks (Home's chip for its project, the open task's: `word`); the list's rows, the sidebar's
// cards and Compare read what's known (`known`), else the harnesses topic. Each worktree task has a folder of its own,
// and a fresh Home asked about 20 times before the first launch (audit C #17).
import { createSignal } from 'solid-js';
import { effect, ownerOf, untracked, type Client, type EntityKey, type PeerId } from '@wisp/client';

const RETRY_MS = 60_000;

export interface DefaultModels {
  /** the model's name, or undefined while it's asked, can't be told, or there's nothing to ask (no folder, Auto) */
  word(peer: string, agent: string, dir: string | undefined): string | undefined;
  /** the folder's answer when it was asked already (a Solid read); never asks */
  known(peer: string, agent: string, dir: string | undefined): string | undefined;
  /** the name a computer's harnesses topic gives its Default (its config there), when it's watched. A Solid read: the
   *  store mirrors the client's rows (an alien effect), so a memo or JSX that reads it follows a late row */
  topic(peer: string, agent: string): string | undefined;
  /** what the picker calls a harness's model ("GPT-6.1-Sol"), from any computer's harnesses topic; a Solid read */
  label(agent: string, model: string): string | undefined;
}

const stores = new WeakMap<object, DefaultModels>();

/** The client's store (made on first use). `now`: the clock for the retry wait (injected in tests). */
export function defaultModels(client: Pick<Client, 'commands' | 'peers'> & Partial<Pick<Client, 'entities'>>, now: () => number = () => performance.now()): DefaultModels {
  const had = stores.get(client);
  if (had) return had;
  // answers by computer, harness and folder: a name, or null when the harness can't tell
  const [answers, setAnswers] = createSignal<ReadonlyMap<string, string | null>>(new Map());
  // in flight (true), or when the last ask failed
  const asked = new Map<string, true | number>();
  const online = (peer: string): boolean => {
    try {
      return client.peers.status(peer as PeerId)().kind === 'online';
    } catch {
      return false;
    }
  };
  // the harnesses topic's words by computer and harness, mirrored for Solid readers (made once, for the client's life)
  const [topics, setTopics] = createSignal<ReadonlyMap<string, string>>(new Map());
  // and the models' labels by harness and model (the same on every computer)
  const [labels, setLabels] = createSignal<ReadonlyMap<string, string>>(new Map());
  const entities = client.entities;
  if (entities) {
    // made on its own (no parent effect it would die with): it lives as long as the client does
    untracked(() => effect(() => {
      const m = new Map<string, string>();
      const l = new Map<string, string>();
      for (const k of entities.keys('runtime.harness')()) {
        const r = entities.row<{ id?: string; agent?: { models?: readonly { id: string; label?: string; resolves?: string }[] } }>('runtime.harness', k as EntityKey)();
        const w = r?.agent?.models?.find((x) => x.id === '')?.resolves;
        if (r?.id && w) m.set(`${ownerOf(k as EntityKey)}\u0000${r.id}`, w);
        for (const x of r?.agent?.models ?? []) if (r?.id && x.id && x.label) l.set(`${r.id}\u0000${x.id}`, x.label);
      }
      setTopics(m);
      setLabels(l);
    }));
  }
  const store: DefaultModels = {
    topic: (peer, agent) => topics().get(`${peer}\u0000${agent}`),
    label: (agent, model) => labels().get(`${agent}\u0000${model}`),
    known: (peer, agent, dir) => (dir && agent ? (answers().get(`${peer}\u0000${agent}\u0000${dir}`) ?? undefined) : undefined),
    word(peer, agent, dir) {
      if (!dir || !agent || agent === 'auto') return undefined;
      const key = `${peer}\u0000${agent}\u0000${dir}`;
      const got = answers().get(key);
      if (got !== undefined) return got ?? undefined;
      const was = asked.get(key);
      if (was === true || (typeof was === 'number' && now() - was < RETRY_MS) || !online(peer)) return undefined;
      asked.set(key, true);
      // the asking is its own chunk (default-model-ask.ts): the answer comes after a request anyway
      void import('./default-model-ask.ts')
        .then((m) => m.askDefault(client, { peer, agent, cwd: dir }))
        .then(
          (v) => {
            asked.delete(key);
            setAnswers((m) => new Map([...m, [key, v ?? null]]));
          },
          () => void asked.set(key, now()),
        );
      return undefined;
    },
  };
  stores.set(client, store);
  return store;
}

/** What Default runs for `agent` on `peer` in `dir`, as the chip says it: the folder's answer when the folder in focus
 *  asked it, else the computer's harnesses topic. Undefined when neither can tell. Never asks (audit C #17).
 *  ShellContext.defaultModel; the list, Compare and the cards read it. */
export function defaultModelName(client: Pick<Client, 'commands' | 'peers' | 'entities'>, peer: string, agent: string, dir: string | undefined): string | undefined {
  const m = defaultModels(client);
  return m.known(peer, agent, dir) ?? m.topic(peer, agent);
}

/** A run's model by name: the one it was given, else what its Default runs (`runs`: defaultModelName), else `words`
 *  ("Default model"). One rule for the task's facts, Compare and the cards. */
export const runModelWord = (model: string | undefined, runs: string | undefined, words: string): string => model || runs || words;
