// Fork… (v1 actions.js forkTask; QA round 14 desktop #1): what the fork should try, its mode and its harness, model
// and thinking among those on the task's machine, then Fork: a copy of the conversation up to the turn (and its files
// then) on a branch of its own, carrying on with what you wrote (`fork.ts`). Nothing is sent before Fork; Cancel, ×
// and Escape leave everything as it was.
import { Show, createResource, createSignal, type JSX } from 'solid-js';
import { defineCommand, entityKey, newRowId, ownerOf, type EntityKey } from '@wisp/client';
import { AutoRunner, Button, Inline, Select, Stack, Text, TextArea, useSignal } from '@wisp/ui';
import { useShell } from '../context.tsx';
import { SESSION, type SessionRow } from '../rows.ts';
import { fork, followUp } from './commands.ts';
import { harnessOf } from './harness.ts';
import { RunnerPicker } from './RunnerPicker.tsx';
import { forkSteps, forkTurn, startPick, type ForkPick } from './fork.ts';
import { longTime } from './rows-view.ts';
import type { AgentOffer } from './runner.ts';
import '../i18n/lazy.ts';

const agentsOf = defineCommand<{ peer: string }, AgentOffer[]>({ id: 'runtime.agents', owner: (a) => a.peer, wire: () => ({}), optimistic: 'none', pending: 'the harness list' });
/** the task's turns, read (the list's Fork… opens with no task page, so the store has none of them: QA round 15 #1) */
const turnsOf = defineCommand<{ key: string; id: string }, readonly { id: string; idx: number; status?: string; started_at?: number; startedAt?: number }[]>({
  id: 'runtime.turns',
  owner: (a) => a.key,
  wire: (a) => ({ id: a.id }),
  optimistic: 'none',
  pending: 'the task’s turns',
});

interface TurnLike {
  readonly id: string;
  readonly session_id: string;
  readonly idx: number;
  readonly status?: string;
  readonly started_at?: number;
}

export default function Fork(props: { readonly params: { readonly id: string; readonly turn?: string } }): JSX.Element {
  const shell = useShell();
  const t = shell.t;
  const key = props.params.id as EntityKey;
  const peer = ownerOf(key);
  const bare = key.slice(key.indexOf(':') + 1);
  const row = useSignal(shell.client.entities.row<SessionRow>(SESSION, key));
  const allTurns = useSignal(shell.client.entities.list<TurnLike>('turn'));
  const harnesses = useSignal(shell.harnesses());
  // what the store has (its page open: live), else what the peer says it has; nothing is decided before that answer
  const [read] = createResource(() =>
    shell.client.commands
      .run(turnsOf, { key, id: bare })
      .then((r) => r.map((x): TurnLike => ({ id: x.id, session_id: bare, idx: x.idx, ...(x.status !== undefined && { status: x.status }), started_at: x.started_at ?? x.startedAt ?? 0 })))
      .catch(() => [] as TurnLike[]),
  );
  const turns = (): TurnLike[] => {
    const own = allTurns().filter((x) => x.session_id === key || x.session_id === bare);
    return (own.length > 0 ? own : (read() ?? [])).toSorted((a, b) => a.idx - b.idx);
  };
  const turn = (): TurnLike | undefined => forkTurn(turns(), props.params.turn);
  // the harnesses its machine has (v1 harnessesAt([place]).filter(available))
  const [offers] = createResource(() => shell.client.commands.run(agentsOf, { peer }).catch(() => [] as AgentOffer[]));
  const [edits, setEdits] = createSignal<ForkPick | undefined>();
  const pick = (): ForkPick | undefined => {
    const s = row();
    if (edits()) return edits();
    if (!s) return undefined;
    const sum = s.summary as { model?: string; effort?: string; mode?: string };
    return startPick({ agent: s.agent, model: sum.model, effort: sum.effort, mode: sum.mode ?? (s as { mode?: string }).mode }, (offers() ?? []).filter((o) => o.available).map((o) => o.id));
  };
  const set = (p: Partial<ForkPick>): void => {
    const cur = pick();
    if (cur) setEdits({ ...cur, ...p });
  };
  const [picking, setPicking] = createSignal(false);
  const close = (): void => shell.navigate({ t: 'closeOverlay', id: 'shell.fork' as never });
  const go = (): void => {
    const p = pick();
    const at = turn();
    if (!p?.prompt.trim()) return;
    const newId = entityKey(peer, newRowId());
    const now = Date.now();
    const steps = forkSteps(p, { of: key, newId, turnId: at?.id, firstTurnId: entityKey(peer, newRowId()), idx: at ? turns().filter((x) => x.idx <= at.idx).length : turns().length, at: now });
    close();
    shell.navigate({ t: 'openTask', id: newId as never });
    // the copy first, then what it should try in it (v1: one request; here two, the second once the copy is there)
    void shell.client.commands
      .run(fork, steps.fork)
      .then(() => shell.client.commands.run(followUp, steps.followUp))
      .catch((err: unknown) => shell.toast.error(t('shell.fork.failed', { why: err instanceof Error ? err.message : String(err) })));
  };
  // named by the time it was asked, as the turn shows it: wisp never numbers turns
  const upTo = (): string => {
    const at = turn();
    return !at || at.id === turns().at(-1)?.id ? t('shell.fork.whole') : t('shell.fork.upTo', { when: longTime(at.started_at ?? 0) });
  };
  return (
    <Show when={pick() && read.state !== 'pending' && read.state !== 'unresolved'} fallback={<Text tone="muted">{t('shell.rerun.loading')}</Text>}>
      {(_ready) => {
        const x = (): ForkPick => pick() as ForkPick;
        return (
        <Stack gap="4" part="fork.body">
          <TextArea
            value={x().prompt}
            onInput={(v) => set({ prompt: v })}
            minRows={3}
            maxRows={12}
            spellcheck={false}
            placeholder={t('shell.fork.prompt')}
            aria-label={t('shell.fork.prompt')}
            autofocus
            onKeyDown={(e: KeyboardEvent) => {
              if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
                e.preventDefault();
                go();
              }
            }}
          />
          <Inline gap="2" align="center" part="fork.bar">
            <Select aria-label={t('shell.rerun.mode')} value={x().mode} onChange={(v) => set({ mode: v })} items={['plan', 'edits', 'auto'].map((m) => ({ value: m, label: t(`shell.mode.${m}`) }))} />
            <AutoRunner
              harness={harnessOf(harnesses(), x().agent).name}
              model={x().model || undefined}
              logo={harnessOf(harnesses(), x().agent).logo}
              title={t('shell.rerun.runner')}
              onClick={() => setPicking(!picking())}
            />
            <span data-kit="grow" />
            <Button variant="ghost" size="sm" onClick={close} part="fork.cancel">
              {t('shell.stop.cancel')}
            </Button>
            <Button variant="primary" size="sm" icon="fork" disabled={!x().prompt.trim() || !turn()} onClick={go} part="fork.go">
              {t('shell.fork.go')}
            </Button>
          </Inline>
          <Text size="sm" tone="muted">
            {turn() ? t('shell.fork.hint', { upTo: upTo(), title: row()?.title ?? '' }) : t('shell.fork.notYet')}
          </Text>
          <RunnerPicker
            open={picking()}
            onOpenChange={setPicking}
            anchor={() => [...document.querySelectorAll('[data-kit="au-runner-btn"]')].at(-1)}
            peer={peer}
            value={{ agent: x().agent, model: x().model, effort: x().effort }}
            onChange={(r) => set({ agent: r.agent, model: r.model, effort: r.effort })}
          />
        </Stack>
        );
      }}
    </Show>
  );
}
