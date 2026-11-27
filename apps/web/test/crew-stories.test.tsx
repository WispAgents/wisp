// @vitest-environment happy-dom
// ADR-0065 step 13b, the crew on phones as stories (docs/design/crew-stories.html C), through the real client, shell
// and plugins (crew, shell-list, shell-task) against the mock peer's crew at work (seed `crew`): Grace asks you
// something, Linus works, Hopper has a report you haven't read, Ada works and has an older report, Turing is paused.
// The phone apps draw the stories row under the Tasks top bar and no Crew section in the list; a phone's browser keeps
// v1's section. The story's answer controls are the task page's own (shell.task:foot), and its reply box sends a
// follow-up to the running job, or starts a job when the member has none.
import { afterEach, expect, test } from 'vitest';
import { createClient, peerId } from '@wisp/client';
import { createShell, nullPlatform, type Platform, type ShellWindow } from '@wisp/shell';
import { createToaster } from '@wisp/ui';
import { COMMANDS } from '../dev/mock-commands.ts';
import { createMockPeer, type MockPeer, type MockTimers } from '../dev/mock-peer.ts';
import { memTransport } from '../dev/mem-transport.ts';
import { pluginUis } from '../src/plugins.ts';
import { until } from './until.ts';

/** the mock's clock without its timers: no streamed turn after a follow-up or a job (what this file doesn't test)
 * repaints the shell under the assertions */
const timers: MockTimers = { now: () => Date.now(), setTimeout: () => undefined, clearTimeout: () => undefined };

let stops: (() => void)[] = [];
afterEach(() => {
  for (const s of stops) s();
  stops = [];
  document.documentElement.removeAttribute('data-platform');
  document.body.innerHTML = '';
});

const PLUGINS = new Set(['wisp.crew', 'wisp.shell-list', 'wisp.shell-task']);

async function start(platform: Platform): Promise<{ root: HTMLElement; mock: MockPeer; sent: { cmd: string; args: Record<string, unknown> }[] }> {
  const mock = createMockPeer({ epoch: 'e1', crew: true, timers });
  // what the peer was asked (each command still runs as the mock answers it)
  const sent: { cmd: string; args: Record<string, unknown> }[] = [];
  for (const cmd of ['runtime.followUp', 'crew.assign', 'runtime.answer']) {
    const fn = COMMANDS[cmd];
    if (fn) mock.handle(cmd, (a, ctx) => (sent.push({ cmd, args: a as Record<string, unknown> }), fn(a, ctx)));
  }
  const home = peerId('local');
  const client = createClient({ clientId: 'c1', build: 't', home, width: 390 });
  const shell = createShell({ client, toaster: createToaster(), platform, home, window: window as unknown as ShellWindow });
  for (const { def } of pluginUis()) if (def && PLUGINS.has(def.id)) shell.register(def);
  await client.start([{ peer: home, name: 'mock', transport: memTransport(mock) }]);
  const root = document.createElement('div');
  document.body.append(root);
  const stop = shell.render(root);
  stops.push(() => {
    stop();
    shell.dispose();
  });
  return { root, mock, sent };
}

const phone = (kind: 'android' | 'ios'): Platform => nullPlatform({ kind, phone: true, mobile: true });
const names = (root: ParentNode): string[] => [...root.querySelectorAll('[data-kit="story-bubble"] [data-kit="story-name"]')].map((e) => e.textContent ?? '');
const rings = (root: ParentNode): string[] => [...root.querySelectorAll<HTMLElement>('[data-kit="story-bubble"]')].map((e) => e.dataset['ring'] ?? '');
/** the row with the seeded crew's busy members in it, their states known: the four working, asking or with a report
 * (UX 2026-11-18 phone #12: idle Turing, Org and Add are the Crew tab's; a member's ring comes from its jobs, the `list`
 * topic's rows, which may land a frame after the crew's) */
const crewRow = (root: ParentNode): boolean => names(root).length === 4 && !rings(root).includes('idle');
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);

// the lazy chunks these tests show (the phone head, the list, the crew's list top and its stories, the task page's
// foot) compiled while the file loads, off the 5 s clock (as dynamic-import-in-test wants): one phone shell up to a
// story's answer card, then gone
{
  const { root } = await start(phone('ios'));
  await until(() => crewRow(root));
  [...root.querySelectorAll<HTMLButtonElement>('[data-kit="story-bubble"]')].find((b) => b.textContent?.includes('Grace'))?.click();
  await until(() => q('[data-part~="crew.story.waiting"] [data-part~="task.foot"]') !== null);
  for (const s of stops) s();
  stops = [];
  document.documentElement.removeAttribute('data-platform');
  document.body.innerHTML = '';
}

async function openStory(root: HTMLElement, name: string): Promise<void> {
  const bubble = [...root.querySelectorAll<HTMLButtonElement>('[data-kit="story-bubble"]')].find((b) => b.textContent?.includes(name));
  expect(bubble, name).toBeDefined();
  bubble?.click();
  await until(() => q(`[data-part~="crew.story.page"] [data-kit="story-who"]`)?.textContent?.includes(name) === true);
}

for (const kind of ['android', 'ios'] as const) {
  test(`on_${kind}_the_crew_is_a_stories_row_under_the_top_bar_ordered_by_ring_and_not_a_section_in_the_list`, async () => {
    const { root } = await start(phone(kind));
    await until(() => crewRow(root));
    const row = root.querySelector('[data-pane="list"] [data-kit="stories"]');
    expect(row, 'the stories row over the list').not.toBeNull();
    // only who needs a look: waiting on you, then the unread report, then working (a job you sent first: the crew's own
    // rank); idle Turing, Org and Add are on the Crew tab (UX 2026-11-18 phone #12)
    expect(names(root)).toEqual(['Grace', 'Hopper', 'Linus', 'Ada']);
    expect(rings(root)).toEqual(['wait', 'report', 'work', 'work']);
    // v1's Crew section is not in the phone app's list
    expect(root.querySelector('[data-part~="section.crew"]')).toBeNull();
  });
}

test('a_phones_browser_keeps_v1s_crew_section_and_draws_no_stories', async () => {
  const { root } = await start(nullPlatform({ mobile: true }));
  // the section with the seeded crew in it (it draws its empty state before the crew's snapshot lands)
  await until(() => root.querySelector('[data-part~="section.crew"]')?.textContent?.includes('Grace') === true);
  expect(root.querySelector('[data-kit="stories"]')).toBeNull();
  // v1's Crew section, in the list
  expect(root.querySelector('[data-part~="section.crew"]')?.textContent).toContain('Grace');
  expect(document.documentElement.hasAttribute('data-platform')).toBe(false);
  // the crew's section in the list, as before (its heading and a row per busy member)
  expect(root.textContent).toContain('Crew');
  expect(root.querySelector('[data-part~="crew.stories.member"]')).toBeNull();
});

test('a_story_shows_what_waits_with_the_task_pages_own_answer_controls_and_steps_through_the_crew', async () => {
  const { root, sent } = await start(phone('android'));
  await until(() => crewRow(root));
  await openStory(root, 'Grace');
  const page = q('[data-part~="crew.story.page"]') as HTMLElement;
  expect(page.querySelector('[data-kit="story-who"]')?.textContent).toContain('Grace');
  const waiting = page.querySelector('[data-part="crew.story.waiting"]');
  expect(waiting, 'Waiting for you').not.toBeNull();
  // the task page's foot (shell-task's TaskFoot: its AskCard), not a copy of it
  await until(() => waiting?.querySelector('[data-part~="task.foot"]') !== null);
  expect(waiting?.querySelector('[data-part~="task.foot"]')).not.toBeNull();
  // only the question and its answers: none of the foot's other extensions (the comments tray, Terminal, Watch)
  expect(waiting?.querySelectorAll('[data-kit="extension"]').length ?? 0).toBeLessThanOrEqual(1);
  expect(waiting?.textContent).toContain('Merge #812 and tag 0.61.0?');
  // the answers are the ask card's numbered options (#995: v1's .q-opts), "1 Yes", not bare buttons
  const yes = [...(waiting?.querySelectorAll<HTMLButtonElement>('[data-part~="task.ask.option"]') ?? [])].find((b) => b.lastElementChild?.textContent?.trim() === 'Yes');
  expect(yes, 'its answer').toBeDefined();
  yes?.click();
  await until(() => sent.some((s) => s.cmd === 'runtime.answer'));
  expect(sent.find((s) => s.cmd === 'runtime.answer')?.args['text']).toBe('Yes');
  // the next member, and the one after, in the row's order as it opened
  (q('[data-part~="crew.story.next"]') as HTMLButtonElement).click();
  await until(() => q('[data-kit="story-who"]')?.textContent?.includes('Hopper') === true);
  expect(q('[data-kit="story-who"]')?.textContent).toContain('Hopper');
  // the stories step through the row as it shows (the busy four)
  expect(q('[data-kit="story-where"]')?.textContent).toBe('2 of 4 · swipe down to close');
  (q('[data-part~="crew.story.close"]') as HTMLButtonElement).click();
  await until(() => q('[data-part~="crew.story.page"]') === null);
  expect(q('[data-part~="crew.story.page"]')).toBeNull();
});

test('the_story_reply_goes_to_the_running_job_and_an_idle_member_is_not_on_the_tasks_strip', async () => {
  const { root, mock, sent } = await start(phone('ios'));
  await until(() => crewRow(root));
  const type = async (text: string): Promise<void> => {
    const box = q('[data-part~="crew.story.reply"] textarea') as HTMLTextAreaElement;
    box.value = text;
    box.dispatchEvent(new InputEvent('input', { bubbles: true }));
    (q('[data-part~="crew.story.reply"] [data-part="composer.send"]') as HTMLButtonElement).click();
  };
  // Linus works: "Message Linus…" is a follow-up on his running job
  await openStory(root, 'Linus');
  expect(q('[data-part~="crew.story.reply"] textarea')?.getAttribute('placeholder')).toBe('Message Linus…');
  await type('Check the iOS bar too');
  await until(() => sent.some((s) => s.cmd === 'runtime.followUp'));
  const linusJob = mock.rows('list').find((r) => (r.v as { title?: string }).title === 'Phone Tasks polish')?.id ?? '';
  expect(sent.find((s) => s.cmd === 'runtime.followUp')?.args).toMatchObject({ id: linusJob, text: 'Check the iOS bar too' });
  expect(sent.some((s) => s.cmd === 'crew.assign')).toBe(false);
  // the box clears at once
  expect((q('[data-part~="crew.story.reply"] textarea') as HTMLTextAreaElement).value).toBe('');
  (q('[data-part~="crew.story.close"]') as HTMLButtonElement).click();
  await until(() => q('[data-part~="crew.story.page"]') === null);
  // Turing has no job: idle members aren't on the Tasks strip (UX 2026-11-18 phone #12); a job for them starts from
  // the Crew tab's member page (crew-phone.test.tsx, New job), and a story that goes idle still assigns (replyTarget)
  expect(names(root)).not.toContain('Turing');
});
