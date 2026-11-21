// The files a prompt went out with show under its words (v1 turns.js .ask-atts): a picture as a thumbnail that opens
// full screen, any other file as a pill that opens it. v2 dropped them: a sent screenshot never showed on the task.
import { expect, test } from 'vitest';
import { mount } from './helpers.tsx';
import { TurnView, type TurnAttachment } from '../src/app/Task.tsx';

const turn = (attachments?: readonly TurnAttachment[]): HTMLElement =>
  mount(() => (
    <TurnView
      id="t1"
      prompt="look at this"
      {...(attachments && { attachments })}
      harness="Claude Code"
      time="12:00"
      timeTitle="12:00"
      acts={[]}
      actsTitle="Steps"
      stats={[]}
      tools={[]}
    />
  ));

test('a_sent_picture_shows_as_a_thumbnail_and_a_file_as_a_pill_under_the_prompt', () => {
  const host = turn([
    { name: 'shot.png', src: '/api/uploads/u1', image: true },
    { name: 'log.txt', src: '/api/uploads/u2' },
    { name: 'gone.pdf' },
  ]);
  const atts = host.querySelector('[data-kit="ask"] [data-kit="ask-atts"]');
  expect(atts, 'the files under the prompt').not.toBeNull();
  const img = atts?.querySelector('[data-kit="ask-att-img"] img');
  expect([img?.getAttribute('src'), img?.getAttribute('alt')]).toEqual(['/api/uploads/u1', 'shot.png']);
  const pills = [...(atts?.querySelectorAll('[data-kit="att-pill"]') ?? [])];
  expect(pills.map((p) => [p.tagName, p.textContent, p.getAttribute('href')])).toEqual([
    ['A', 'log.txt', '/api/uploads/u2'],
    ['SPAN', 'gone.pdf', null],
  ]);
});

test('a_prompt_with_no_files_has_no_files_row', () => {
  expect(turn().querySelector('[data-kit="ask-atts"]')).toBeNull();
  expect(turn([]).querySelector('[data-kit="ask-atts"]')).toBeNull();
});
