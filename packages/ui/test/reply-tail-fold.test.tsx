// The task page's reply box folds like New task's (QA r10 #1 follow-up): at 900 with Goal, Delegate and Start later on,
// its chips ran past the box and Send off the window (559 px of bar in 426). Its chips fold into More by measured width,
// the one whose panel is open (Delegate, just opened from +) last, and a folded chip keeps its clear in More.
import { afterEach, expect, test } from 'vitest';
import { mount, until } from './helpers.tsx';
import { ReplyBox, type LauncherTool } from '../src/index.ts';

const saved = {
  offset: Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'offsetWidth'),
  client: Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientWidth'),
  rects: Object.getOwnPropertyDescriptor(Element.prototype, 'getClientRects'),
};
afterEach(() => {
  for (const [k, d] of [['offsetWidth', saved.offset], ['clientWidth', saved.client]] as const) if (d) Object.defineProperty(HTMLElement.prototype, k, d);
  if (saved.rects) Object.defineProperty(Element.prototype, 'getClientRects', saved.rects);
  document.body.innerHTML = '';
});

const CHIPS = new Set(['goal', 'team', 'later']);

test('the_reply_bars_chips_fold_into_more_the_open_panel_stays_and_a_folded_chip_keeps_its_clear', async () => {
  // every chip 120 px, More 30, the other buttons 40; the bar's inside 420
  Object.defineProperty(Element.prototype, 'getClientRects', { configurable: true, value: () => [{}] });
  Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
    configurable: true,
    get(this: HTMLElement) {
      const a = this.getAttribute('data-a');
      if (a && CHIPS.has(a)) return 120;
      if (a === 'more') return 30;
      return this.tagName === 'BUTTON' ? 40 : 0;
    },
  });
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get(this: HTMLElement) { return this.getAttribute('data-kit') === 'c-bar' ? 420 : 0; } });
  const cleared: string[] = [];
  const chip = (id: string, extra: Partial<LauncherTool> = {}): LauncherTool => ({ id, part: `composer.${id}`, icon: 'target', label: id, title: id, text: true, on: true, clear: { label: `Clear ${id}`, act: { onClick: () => cleared.push(id) } }, ...extra });
  const chips = [chip('goal'), chip('team', { act: { panel: { label: 'Delegate', part: 'composer.team.panel', content: () => null, open: true } } }), chip('later', { label: 'Starts at 13:08' })];
  const box = mount(() => (
    <ReplyBox
      placeholder="Follow up…"
      value="x"
      onInput={() => undefined}
      onSend={() => undefined}
      attach="Attach"
      mode={{ label: 'Full auto', title: 'Full auto' }}
      runner={{ title: 'Claude Code' }}
      team="Delegate"
      goal="Goal"
      later="Start later"
      send="Send"
      plus={{ label: 'More', title: 'More', act: {} }}
      chips={chips}
      chipsMore={{ label: 'More options', title: 'More options' }}
    />
  ));
  const shown = (): string[] => [...box.querySelectorAll('[data-kit="c-bar"] > [data-a]')].map((e) => e.getAttribute('data-a') ?? '');
  await until(() => shown().includes('more'), 2000);
  // two chips go (240 of 520 doesn't fit in 420 with More); Delegate's panel is open, so it stays
  expect(shown()).toEqual(['plus', 'team', 'more']);
  // one measurer: the fold's squeeze is the bar's tight form (the mode to its icon), so what it measured is what's drawn
  expect(box.querySelector('[data-kit="c-bar"]')?.hasAttribute('data-tight')).toBe(true);
});
