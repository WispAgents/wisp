// UX 2026-11-18 desktop #7: every turn's footer showed six facts and five icons. At rest it says the time and the cost;
// the steps, the receipt and the icons come on hover or focus within the turn (opacity only: nothing moves), always on
// the latest turn's icons, and always on a touch screen. Every feature stays reachable.
import { expect, test } from 'vitest';
import CSS from '../src/css/app-task.css?raw';
import { mount } from './helpers.tsx';
import { TurnView } from '../src/app/Task.tsx';

const turn = (latest: boolean): HTMLElement =>
  mount(() => (
    <TurnView
      id="t1"
      prompt="fix it"
      harness="Claude Code"
      time="12:00"
      timeTitle="12:00"
      acts={[{ text: '16' }]}
      actsTitle="Steps"
      stats={[{ text: '2m 29s' }, { text: '$0.93' }]}
      receipt={{ text: '6623 tokens' }}
      tools={[{ id: 'notes', icon: 'text', label: 'Notes' }]}
      latest={latest}
    />
  ));

test('a_turns_time_and_cost_are_its_facts_at_rest_and_it_says_if_it_is_the_latest', () => {
  const host = turn(true);
  expect([...host.querySelectorAll('[data-kit="tb"][data-a="stat"]')].map((e) => e.textContent)).toEqual(['2m 29s', '$0.93']);
  expect(host.querySelector('[data-kit="turn"]')?.hasAttribute('data-latest')).toBe(true);
  expect(turn(false).querySelector('[data-kit="turn"]')?.hasAttribute('data-latest')).toBe(false);
});

test('the_rest_of_the_footer_shows_on_hover_or_focus_and_the_latest_turns_icons_always', () => {
  const quiet = CSS.match(/\/\* UX 2026-11-18 #7[\s\S]*?(?=\n {2}\/\*(?! the)|\n}\s*$)/)?.[0] ?? '';
  expect(quiet, 'the quiet-footer rules').not.toBe('');
  expect(quiet).toMatch(/:not\(:hover\):not\(:focus-within\)/);
  expect(quiet).toMatch(/opacity:\s*0/);
  expect(quiet).toMatch(/\[data-latest\]/);
  expect(quiet).toMatch(/@media \(hover: hover\)/);
  expect(quiet).toMatch(/\[data-a='stat'\][^{]*\{[^}]*order:\s*-1/);
});
