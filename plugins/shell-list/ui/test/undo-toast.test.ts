// QA round 4 desktop #3: Undo on "Saved for later" showed a second toast, "Unsaved · Undo", which stayed up (and stacked
// under the next action's). An Undo puts the task back and says nothing new; its own toast closes with the press.
import { expect, test } from 'vitest';
import type { EntityKey, PluginClient } from '@wisp/client';
import type { Toaster } from '@wisp/ui';
import { toggleArchived, toggleDone, toggleForLater } from '../src/actions.ts';
import type { SessionRow } from '../src/types.ts';

const key = 'local:t1' as EntityKey;

function ctx(row: SessionRow) {
  const sent: { cmd: string; args: unknown }[] = [];
  const toasts: { text: string; undo?: () => void }[] = [];
  let now = row;
  const client = {
    entities: { peek: () => now },
    navigate: () => {},
    run: (cmd: { id: string }, args: Record<string, unknown>) => {
      sent.push({ cmd: cmd.id, args });
      // the row as the optimistic patch leaves it
      if (cmd.id === 'runtime.forLater') now = { ...now, forLater: args['forLater'] ? { at: 1000, by: 'you' } : undefined } as SessionRow;
      if (cmd.id === 'runtime.setState') now = { ...now, state: args['state'] } as SessionRow;
      if (cmd.id === 'runtime.archive') now = { ...now, archived: args['archived'] } as SessionRow;
      return Promise.resolve();
    },
  } as unknown as PluginClient;
  const push = (text: string, o?: { action?: { onClick: () => void } }): void => void toasts.push({ text, ...(o?.action && { undo: o.action.onClick }) });
  const toast = { show: push, success: push } as unknown as Toaster;
  return { c: { client, t: (k: string) => k, toast, now: () => 1000 }, sent, toasts };
}

const finished = { id: 't1', title: 'T', agent: 'claude', state: 'review', createdAt: 0, updatedAt: 0 } as SessionRow;

for (const [name, act, back] of [
  ['save for later', toggleForLater, (s: SessionRow) => !s.forLater],
  ['mark done', toggleDone, (s: SessionRow) => s.state === 'review'],
  ['archive', toggleArchived, (s: SessionRow) => !s.archived],
] as const) {
  test(`undo_of_${name.replace(/ /g, '_')}_puts_it_back_and_shows_no_new_toast`, () => {
    const { c, toasts, sent } = ctx(finished);
    act(c, key);
    expect(toasts).toHaveLength(1);
    toasts[0]?.undo?.();
    expect(toasts.map((t) => t.text), 'nothing new after Undo').toHaveLength(1);
    // the task is back as it was
    const last = sent.at(-1);
    expect(last).toBeDefined();
    const row = (c.client.entities as unknown as { peek: () => SessionRow }).peek();
    expect(back(row)).toBe(true);
  });
}
