// wisp.shell-task's UI, the part only a press reaches (ADR-0067: PluginUiDef.later): loaded as the first screen draws.
import { defineCommand, uuidv7 } from '@wisp/client';
import type { PluginUiLater } from '@wisp/shell';
import { defineExtension } from './define.ts';
import { WATCH_VIEW } from './open-task.ts';
import { added, commentStore, marksOn, waiting, withComments } from './comments.ts';
import { CATALOGUE } from './i18n/index.ts';
import { appRunning, runRequest } from './side/run-request.ts';
import { filesSource } from './side/open-file.ts';

/** Stop app (v1 stopRun): wisp.terminal closes the task's run */
const stopRun = defineCommand<{ key: string; id: string }, { ok: boolean }>({
  id: 'terminal.close',
  owner: (a) => a.key as never,
  wire: (a) => ({ kind: 'run', id: a.id }),
  optimistic: 'none',
  pending: 'Stop app',
});

const later: PluginUiLater = {
  extensions: [
    // Run app / Stop app in a task's row menu (v1 panel.js runItem, runApp): a task with a worktree. While its app runs
    // (the row's mark from wisp.terminal) it stops it; else the task's Preview opens and starts it with its command
    defineExtension({
      id: 'shell-task.rowRun',
      attachTo: { id: 'shell.row:menu' },
      component: () => null,
      order: 80,
      if: (p) => {
        const s = p.session?.() as { branch?: string; pending?: boolean } | undefined;
        return !!s?.branch && !s.pending;
      },
      bar: {
        icon: 'play',
        label: 'shell-task.side.runApp',
        priority: 80,
        sub: 'view',
        text: (p) => CATALOGUE[appRunning(p.session?.() as never) ? 'shell-task.side.stopApp' : 'shell-task.side.runApp'] ?? '',
        select: (p) => {
          const s = p.session?.() as { id?: string; ext?: Readonly<Record<string, unknown>> } | undefined;
          const key = String(s?.id ?? '');
          if (!key) return;
          if (appRunning(s)) return void p.client.run(stopRun, { key: key as never, id: key.slice(key.indexOf(':') + 1) }).catch(() => undefined);
          runRequest(key.slice(key.indexOf(':') + 1));
          p.client.navigate({ t: 'openTask', id: key as never });
          p.client.navigate({ t: 'openSide', side: { kind: 'preview', id: key as never } });
        },
      },
    }),
    // Watch in a task's row menu (v1 panel.js watchItem): a task whose agent keeps a page beside it (wisp_watch)
    defineExtension({
      id: 'shell-task.rowWatch',
      attachTo: { id: 'shell.row:menu' },
      component: () => null,
      order: 84,
      if: (p) => !!(p.session?.() as { watch?: unknown } | undefined)?.watch,
      bar: {
        icon: 'eye',
        label: 'shell-task.side.watch',
        priority: 84,
        sub: 'view',
        select: (p) => {
          const key = String((p.session?.() as { id?: string } | undefined)?.id ?? '');
          if (!key) return;
          p.client.navigate({ t: 'openTask', id: key as never });
          p.client.navigate({ t: 'openSide', side: { kind: 'view', id: WATCH_VIEW as never } });
        },
      },
    }),
  ],
  // a terminal task's CLI beside it (`shell-task.term`, the Terminal tab)
  // the comments go at the top of the next follow-up (v1 comment-text.js withComments)
  prompt: [
    {
      id: 'shell-task.comments',
      trigger: '/',
      where: ['reply'],
      order: 99,
      items: () => [],
      // comments on the task's answers (v1 comments.js): kept with the line ones, numbered with them
      answer: {
        add: (c, ctx) => commentStore(ctx.client).set((cur) => added(cur, ctx.task, { ...c, id: uuidv7() }) as never),
        marks: (turn, ctx) => marksOn(waiting(commentStore(ctx.client).value(), ctx.task), turn),
      },
      // what Ship and Merge warn about: comments (on lines of Changes, on answers) not sent yet
      waiting: (ctx) => waiting(commentStore(ctx.client).value(), ctx.task).length,
      pending: (text, ctx) => {
        const store = commentStore(ctx.client);
        const list = waiting(store.value(), ctx.task);
        if (list.length === 0) return undefined;
        const was = store.value();
        return {
          text: withComments(list, text),
          taken: () =>
            store.set((cur) => {
              const next = { ...(cur as Record<string, unknown>) };
              delete next[ctx.task];
              return next as never;
            }),
          back: () => store.set(was as never),
        };
      },
    },
  ],
  // Changes' Open file…: the palette on the task's files alone, then the words inside them (v1 openFileFinder)
  palette: [filesSource],
};
export default later;
