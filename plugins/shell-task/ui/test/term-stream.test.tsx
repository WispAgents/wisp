// A terminal's event stream that ends without saying the program did (the server dropped a page that fell behind,
// the connection went) is a disconnect: the pane attaches again for the whole screen, and shows the end when the
// attach answers it. It sat there, the screen frozen, as if the program still ran. And a shell started again numbers
// its keys afresh: the page kept counting from the last one and every key was held as a gap.
import { expect, test } from 'vitest';
import type { JSX } from 'solid-js';
import type { CommandDef } from '@wisp/client';
import { ShellPane } from '../src/side/ShellPane.tsx';
import { PreviewPane } from '../src/side/PreviewPane.tsx';
import { useTask } from '../src/task-context.tsx';
import { mountTask, until } from './helpers.tsx';

type Ev = { event: string; data: string };
type Stream = (signal: AbortSignal) => AsyncGenerator<Ev>;
const hello: Ev = { event: 'hello', data: JSON.stringify({ cols: 100, rows: 30, resizable: true, state: 'running' }) };
const data = (s: string): Ev => ({ event: 'data', data: JSON.stringify(s) });
/** open until the page lets go */
const held = (signal: AbortSignal): Promise<void> => new Promise((r) => signal.addEventListener('abort', () => r()));

/** mounts `view` with `streams` answering the attaches in turn; `ran` the commands it ran with their args */
async function mount(view: () => JSX.Element, streams: Stream[], answers: Record<string, unknown>, ran: Array<[string, Record<string, unknown>]>): Promise<{ attaches: () => number }> {
  let n = 0;
  await mountTask({ sessionId: 's1', rows: {} }, () => {
    const task = useTask();
    (task.client as { run: unknown }).run = (cmd: CommandDef<unknown, unknown>, a: Record<string, unknown>) => {
      ran.push([cmd.id, a]);
      return Promise.resolve(answers[cmd.id] ?? { ok: true });
    };
    (task.client as { http: unknown }).http = () => ({
      events: (_path: string, signal: AbortSignal) => {
        const s = streams[Math.min(n, streams.length - 1)];
        n += 1;
        return s?.(signal);
      },
    });
    return view();
  });
  return { attaches: () => n };
}
const shellAnswers = {
  'terminal.shell.info': { supported: true, open: false, app: true, name: 'zsh' },
  'terminal.shell.start': { ok: true, started: true },
};

test('a_shell_stream_that_ends_without_exit_attaches_again_and_shows_the_end_it_answers', async () => {
  const ran: Array<[string, Record<string, unknown>]> = [];
  const m = await mount(
    () => <ShellPane kind="shell" />,
    [
      async function* () {
        yield hello;
        yield data('one');
      },
      async function* () {
        yield hello;
        yield data('one two');
        yield { event: 'exit', data: JSON.stringify({ code: 3 }) };
      },
    ],
    shellAnswers,
    ran,
  );
  await until(() => m.attaches() >= 2 && document.body.textContent.includes('exit 3'));
  expect(m.attaches()).toBe(2);
  expect(document.body.textContent).toContain('Shell ended (exit 3)');
  await new Promise((r) => setTimeout(r, 400));
  expect(m.attaches(), 'ended: no further attach').toBe(2);
});

test('a_preview_stream_that_ends_without_exit_attaches_again_and_a_gone_answer_shows_it_ended', async () => {
  const ran: Array<[string, Record<string, unknown>]> = [];
  const m = await mount(
    () => <PreviewPane />,
    [
      async function* () {
        yield hello;
        yield data('ready');
      },
      async function* () {
        yield { event: 'gone', data: '{}' };
      },
    ],
    { 'terminal.run.info': { open: true, port: 3000, dir: '/tmp' } },
    ran,
  );
  await until(() => m.attaches() >= 2 && document.body.textContent.includes('Stopped'));
  expect(m.attaches()).toBe(2);
  expect(document.body.textContent).toContain('Stopped');
});

test('a_shell_started_again_numbers_its_keys_afresh', async () => {
  const ran: Array<[string, Record<string, unknown>]> = [];
  let end: () => void = () => undefined;
  const ended = new Promise<void>((r) => (end = r));
  await mount(
    () => <ShellPane kind="shell" />,
    [
      async function* () {
        yield hello;
        yield data('$ ');
        await ended;
        yield { event: 'exit', data: JSON.stringify({ code: 0 }) };
      },
      async function* (signal) {
        yield hello;
        yield data('$ ');
        await held(signal);
      },
    ],
    shellAnswers,
    ran,
  );
  const key = (k: string): void => void document.querySelector('[data-kit="term-input"]')?.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true }));
  const keys = (): Array<Record<string, unknown>> => ran.filter(([id, a]) => id === 'terminal.input' && a.data !== undefined).map(([, a]) => a);
  await until(() => !!document.querySelector('[data-kit="term-input"]') && document.body.textContent.includes('zsh'));
  key('Enter');
  key('Tab');
  await until(() => keys().length === 2);
  expect(keys().map((a) => a.seq)).toEqual([0, 1]);
  end();
  await until(() => document.body.textContent.includes('Start again'));
  [...document.querySelectorAll('button')].find((b) => b.textContent.includes('Start again'))?.click();
  await until(() => ran.filter(([id, a]) => id === 'terminal.shell.start' && a.restart).length === 1 && !document.body.textContent.includes('Start again'));
  key('Enter');
  await until(() => keys().length === 3);
  const [first, , again] = keys();
  expect(again?.seq, 'the new shell counts from 0').toBe(0);
  expect(again?.from, 'as a page it never heard of').not.toBe(first?.from);
});
