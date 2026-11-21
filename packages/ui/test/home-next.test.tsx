// Home's Needs you as an inbox (TRY-27, src/app/Home.tsx NextList): each task with why it needs you (the question it
// asks, else the start of its answer) and its status in its tone; v1's title-only rows otherwise.
import { expect, test } from 'vitest';
import { mount } from './helpers.tsx';
import { NextList } from '../src/app/Home.tsx';

test('a needs-you row says why under its title and its status in its tone', () => {
  const opened: string[] = [];
  const host = mount(() => (
    <NextList
      heading="Needs you"
      count={2}
      rows={[
        { id: 'a', title: 'Typo fix', color: 'slate', state: 'Finished', tone: 'done', why: 'No typo, so I added the URL sentence', time: '5m', timeTitle: '' },
        { id: 'b', title: 'Greeting', color: 'slate', state: 'Report', tone: 'info', time: '3h', timeTitle: '' },
      ]}
      onOpen={(id) => opened.push(id)}
    />
  ));
  const rows = [...host.querySelectorAll<HTMLElement>('[data-kit="sh-task"]')];
  expect(rows.map((r) => r.querySelector('[data-kit="sh-why"]')?.textContent ?? null)).toEqual(['No typo, so I added the URL sentence', null]);
  expect(rows.map((r) => r.querySelector('[data-kit="sh-s"]')?.getAttribute('data-tone'))).toEqual(['done', 'info']);
  rows[1]?.click();
  expect(opened).toEqual(['b']);
});

// QA round 1 #14: "Needs you 9" over five rows, no word of the rest. v1 paintHome: "5 of 9" when there are more.
test('needs you says how many it shows of how many when it shows fewer', () => {
  const row = (id: string) => ({ id, title: id, color: 'slate', state: 'Report', tone: 'info' as const, time: '1m', timeTitle: '' });
  const host = mount(() => <NextList heading="Needs you" count={9} countLabel="5 of 9" rows={['a', 'b', 'c', 'd', 'e'].map(row)} onOpen={() => {}} />);
  expect(host.querySelector('[data-kit="sec-n"]')?.textContent).toBe('5 of 9');
});

// QA round 2 desktop #2: "Needs you 0" stood alone on Home. v1 paintHome hid the section when nothing needed you.
test('needs you draws nothing when nothing needs you', () => {
  const host = mount(() => <NextList heading="Needs you" count={0} rows={[]} onOpen={() => {}} />);
  expect(host.querySelector('[data-kit="sh-next"]')).toBeNull();
});
