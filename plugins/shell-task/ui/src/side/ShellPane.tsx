// The side panel's Shell (v1 features/panel.js startShell / paintShell / endShell): a login shell in the task's
// folder on the computer it runs on, in the kit's <Terminal>. Starting shows what starts where; a computer that
// can't run one says so in one sentence; an ended shell keeps its last screen with Start again (never the dead
// screen reopened); End shell hangs it up. Keys and sizes go through wisp.terminal's commands (`terminal.input`,
// numbered so they go in in order); the output is its event stream (`GET /api/sessions/:id/shell`: hello, data,
// exit, gone), read through the client's `http(peer).events`, so a machine's or the grid's task works the same.
// `kind="takeover"`: a terminal task's CLI (v1 services/tui.js), opened by `runtime.terminal` (on its prompt the
// first time, resumed after) and streamed from `/sessions/:id/term`; ending it leaves the task for review.
import { Show, createSignal, onCleanup, onMount, type JSX } from 'solid-js';
import { defineCommand, ownerOf, type EntityKey } from '@wisp/client';
import { Button, EmptyState, Inline, Screen, Stack, Terminal, Text } from '@wisp/ui';
import { useShell } from '../shell.ts';
import { useTask } from '../task-context.tsx';
import '../i18n/lazy.ts';

const bare = (k: string): string => k.slice(k.indexOf(':') + 1);
type Info = { supported: boolean; open: boolean; app: boolean; name: string };
const info = defineCommand<{ key: EntityKey; id: string }, Info>({ id: 'terminal.shell.info', owner: (a) => a.key, wire: (a) => ({ id: a.id }), optimistic: 'none', pending: 'the Shell pane' });
const start = defineCommand<{ key: EntityKey; id: string; cols: number; rows: number; restart?: boolean }, { ok: boolean; started: boolean; dir?: string; gone?: boolean }>({
  id: 'terminal.shell.start',
  owner: (a) => a.key,
  wire: (a) => ({ id: a.id, cols: a.cols, rows: a.rows, ...(a.restart && { restart: true }) }),
  optimistic: 'none',
  pending: 'the Shell pane',
});
const tuiStart = defineCommand<{ key: EntityKey; id: string; cols: number; rows: number }, { state?: string }>({
  id: 'runtime.terminal',
  owner: (a) => a.key,
  wire: (a) => ({ id: a.id, cols: a.cols, rows: a.rows }),
  optimistic: 'none',
  pending: 'the Terminal pane',
});
type Kind = 'shell' | 'takeover';
// a phone (UX 2026-11-18 phone #14): the pane says why it can't start, in words, where the terminal goes; no toast with
// a raw path over it (the browser keeps its toast)
const infoHere = defineCommand({ ...info, quiet: true });
const startHere = defineCommand({ ...start, quiet: true });
const tuiStartHere = defineCommand({ ...tuiStart, quiet: true });
const input = defineCommand<{ key: EntityKey; kind: Kind; id: string; data?: string; seq?: number; from?: string; cols?: number; rows?: number }, { ok: boolean }>({
  id: 'terminal.input',
  owner: (a) => a.key,
  wire: ({ key: _k, ...a }) => a,
  optimistic: 'none',
  pending: 'none: keys go in as typed',
});
const inputHere = defineCommand({ ...input, quiet: true });
const close = defineCommand<{ key: EntityKey; kind: Kind; id: string }, { ok: boolean }>({
  id: 'terminal.close',
  owner: (a) => a.key,
  wire: (a) => ({ kind: a.kind, id: a.id }),
  optimistic: 'none',
  pending: 'End shell',
});

export function ShellPane(props: { readonly kind: Kind }): JSX.Element {
  const shell = useShell();
  const task = useTask();
  const t = shell.t;
  const c = task.client;
  const id = bare(task.key);
  const tui = props.kind === 'takeover';
  const screen = new Screen(100, 30);
  const from = Math.random().toString(36).slice(2, 10);
  let seq = 0;
  let es: AbortController | undefined;
  const [where, setWhere] = createSignal<Info | undefined>();
  const [error, setError] = createSignal('');
  const phone = shell.platform.phone;
  /** a refusal in words: a gone folder says so on a phone; anything else as the peer said it */
  const say = (e: unknown): string =>
    phone && (e as { code?: unknown }).code === 'not_found' ? t('shell-task.shell.gone') : e instanceof Error ? e.message : String(e);
  const [drawn, setDrawn] = createSignal(false);
  const [ended, setEnded] = createSignal<number | null | undefined>(undefined);
  const [size, setSize] = createSignal({ cols: 100, rows: 30 });

  const connect = (): void => {
    es?.abort();
    const ac = new AbortController();
    es = ac;
    // the task's peer's own route, through its transport (this computer, a machine, the grid)
    void (async () => {
      try {
        for await (const ev of c.http(ownerOf(task.key)).events(`/sessions/${encodeURIComponent(id)}/${tui ? 'term' : 'shell'}`, ac.signal)) {
          if (ev.event === 'hello') {
            const h = JSON.parse(ev.data) as { cols: number; rows: number };
            screen.reset();
            screen.resize(h.cols, h.rows);
            setEnded(undefined);
          } else if (ev.event === 'data') {
            screen.write(JSON.parse(ev.data) as string);
            setDrawn(true);
          } else if (ev.event === 'exit' || ev.event === 'gone') {
            setEnded(ev.event === 'exit' ? ((JSON.parse(ev.data) as { code: number | null }).code ?? 0) : 0);
            ac.abort();
          }
        }
      } catch (e: unknown) {
        if (!ac.signal.aborted) setError(say(e));
      }
    })();
  };
  // the terminal mounts the frame after the panel's first: measuring and painting it in the open put 100 ms of forced
  // layout and paint in the panel's frame at 4× (perf r1 #8: 194 ms to the panel, 285 ms in WebKit). The panel, its tabs
  // and the screen's box show at once; the shell starts once the terminal has measured its box, so it starts at the
  // pane's size, not the 100×30 default
  const [mounted, setMounted] = createSignal(typeof requestAnimationFrame !== 'function');
  let measured: (() => void) | undefined;
  if (!mounted()) {
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => {
        setMounted(true);
        measured?.();
        measured = undefined;
      });
    });
    onCleanup(() => cancelAnimationFrame(frame));
  }
  const begin = (restart = false): void => {
    setError('');
    setDrawn(false);
    setEnded(undefined);
    const go = tui ? c.run(phone ? tuiStartHere : tuiStart, { key: task.key, id, ...size() }) : c.run(phone ? startHere : start, { key: task.key, id, ...size(), ...(restart && { restart: true }) });
    go.then(connect, (e: unknown) => setError(say(e)));
  };
  onMount(() => {
    c.run(phone ? infoHere : info, { key: task.key, id }).then(
      (i) => {
        setWhere(i);
        if (!i.supported) return;
        if (mounted()) begin();
        else measured = begin;
      },
      (e: unknown) => setError(say(e)),
    );
  });
  onCleanup(() => es?.abort());
  const send = (data: string): void => {
    if (ended() !== undefined) return;
    // numbered from 0 per page (wisp_terminal_core::order): they go in in order whatever order they land in
    const n = seq;
    seq += 1;
    void c.run(phone ? inputHere : input, { key: task.key, kind: props.kind, id, data, seq: n, from }).catch(() => undefined);
  };
  const resize = (cols: number, rows: number): void => {
    setSize({ cols, rows });
    if (ended() === undefined) void c.run(phone ? inputHere : input, { key: task.key, kind: props.kind, id, cols, rows }).catch(() => undefined);
  };
  const end = (): void => {
    setEnded(0);
    es?.abort();
    void c.run(close, { key: task.key, kind: props.kind, id }).catch(() => undefined);
  };
  const name = (): string => (tui ? (task.task()?.agent ?? '') : (where()?.name ?? ''));
  const peerName = (): string => (ownerOf(task.key) === c.home ? t('shell-task.shell.here') : String(c.peers.name?.(ownerOf(task.key)) ?? ''));

  return (
    <Stack gap="2" part="side.shell" grow>
      <Show
        when={!error() && where()?.supported !== false}
        fallback={
          <EmptyState
            icon={error() ? 'warn' : 'terminal'}
            title={error() || t('shell-task.shell.unsupported')}
            compact
            action={
              <Button size="sm" variant="ghost" onClick={() => (error() ? begin() : undefined)}>
                {t('shell-task.shell.retry')}
              </Button>
            }
          />
        }
      >
        <Stack gap="0" grow part="side.shell.screen">
          <Show when={mounted()}>
          <Terminal
            screen={screen}
            onData={send}
            onResize={resize}
            ended={ended() !== undefined}
            onEndedKey={() => begin(true)}
            {...(!drawn() && { waiting: where() ? t('shell-task.shell.starting', { name: name() || 'a shell', where: peerName() }) : t('shell-task.shell.finding', { where: peerName() }) })}
            labels={{ input: t('shell-task.shell.input'), keys: t('shell-task.shell.keys'), paste: t('shell-task.shell.paste') }}
          />
          </Show>
        </Stack>
        <Inline gap="2" justify="between" part="side.shell.foot">
          <Text size="sm" tone="muted" truncate>
            {ended() !== undefined
              ? ended()
                ? t(tui ? 'shell-task.term.endedCode' : 'shell-task.shell.endedCode', { code: ended() ?? 0 })
                : t(tui ? 'shell-task.term.ended' : 'shell-task.shell.ended')
              : name()}
          </Text>
          <Show
            when={ended() !== undefined}
            fallback={
              <Button size="xs" variant="ghost" icon="x" onClick={end}>
                {t(tui ? 'shell-task.term.end' : 'shell-task.shell.end')}
              </Button>
            }
          >
            <Button size="xs" variant="ghost" icon="replay" onClick={() => begin(true)}>
              {t('shell-task.shell.again')}
            </Button>
          </Show>
        </Inline>
      </Show>
    </Stack>
  );
}
