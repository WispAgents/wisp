// wisp.later's UI, the part only a press reaches (ADR-0067: PluginUiDef.later): loaded as the first screen draws.
import { uuidv7, type EntityKey } from '@wisp/client';
import type { PluginUiLater } from '@wisp/shell';
import { addFollowUp, addPlan } from './commands.ts';
import { plannedSaid } from './format.ts';
import { launchPlan } from './launch.ts';
import { replyPlan, type ReplyTask } from './reply.ts';

const later: PluginUiLater = {
  // New task's Send, when it means later: a time on the clock or at the start of the prompt, /draft, /after <task>
  prompt: [
    {
      id: 'later.launch',
      trigger: '/',
      where: ['home'],
      order: 90,
      items: () => [],
      send: (text, ctx) => {
        const l = (ctx.launch ?? {}) as { cwd?: string; projectId?: string; worktree?: boolean; agent?: string; model?: string; effort?: string; mode?: string; at?: number };
        const home = ctx.client.home;
        const key = (id: string): EntityKey => (id.includes(':') ? id : `${home}:${id}`) as EntityKey;
        const read = ctx.client.entities.peek<{ value?: unknown }>('setting', `${home}:settings.readTimes` as EntityKey)?.value;
        const p = launchPlan(text, {
          now: new Date(),
          at: l.at,
          readTimes: read !== false && read !== 'off',
          title: (id) => ctx.client.entities.peek<{ title?: string }>('session', key(id))?.title,
        });
        if (!p) return false;
        const body = {
          prompt: p.rest,
          ...(l.cwd && { cwd: l.cwd }),
          // where it starts, as New task said: in a worktree of its own or in place, and its project for the list
          ...(l.cwd && l.worktree !== undefined && { worktree: l.worktree }),
          ...(l.projectId && { projectId: l.projectId }),
          ...(l.agent && { agent: l.agent }),
          ...(l.model && { model: l.model }),
          ...(l.effort && { effort: l.effort }),
          ...(l.mode && { mode: l.mode }),
        };
        const base = { peer: home, tmp: uuidv7(), setAt: Date.now() };
        const args =
          'draft' in p
            ? { ...base, kind: 'task' as const, draft: true as const, body }
            : 'after' in p
              ? { ...base, kind: 'task' as const, after: { id: key(p.after.id), title: p.after.title }, body }
              : p.kind === 'reminder'
                ? { ...base, kind: 'reminder' as const, at: p.at, text: p.rest }
                : { ...base, kind: 'task' as const, at: p.at, body };
        void ctx.client.run(addPlan, args).catch(() => undefined);
        // where it went, as v1 said it: the Scheduled row may be out of sight (audit B #6)
        ctx.say?.(plannedSaid(p, Date.now()));
        return true;
      },
    },
    // a task's reply that starts with a time (v1 readWhen, F-auto-5): the follow-up goes into the same conversation
    // then; its row shows above the box at once (later.add's optimistic patch), back with a toast if refused
    {
      id: 'later.reply',
      trigger: '/',
      where: ['reply'],
      order: 95,
      items: () => [],
      send: (text, ctx) => {
        const home = ctx.client.home;
        const task = ctx.task as EntityKey;
        const setting = ctx.client.entities.peek<{ value?: unknown }>('setting', `${home}:settings.readTimes` as EntityKey);
        // a time picked on the box's clock (Send later, or the hold row's Wait until the reset), else one typed in it
        const picked = (ctx.launch as { at?: number } | undefined)?.at;
        const p =
          picked !== undefined && picked > Date.now() && text.trim()
            ? { at: picked, text: text.trim(), remind: false }
            : replyPlan(text, { now: new Date(), task: ctx.client.entities.peek<ReplyTask>('session', task), readTimes: setting?.value !== false && setting?.value !== 'off' });
        if (!p) return false;
        // refused: the row goes (the patch rolls back, with the toast) and what was typed is back in the box
        void ctx.client.run(addFollowUp, { sessionId: task, at: p.at, text: p.text, tmp: uuidv7(), setAt: Date.now(), ...(p.remind && { remind: true }) }).catch(() => ctx.restore?.(text));
        return true;
      },
    },
  ],
};
export default later;
