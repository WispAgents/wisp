// Audit A #8: a swipe past the line runs its action. The row flies off, closes, and onSwipe runs, whatever the look's
// transitions are: no `transitionend` is waited on (a step with no transition, or a cancelled one, never sends one, and
// the row sat at `closing` with nothing run). Each step waits for the row's own running animations, and goes on at
// once when there are none. The row is left idle, ready for the list to move or drop it.
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { JSX } from 'solid-js';
import { mount, until } from './helpers.tsx';
import { ListRow, PlatformLookProvider, Swipe, type Look, type SwipeDir } from '../src/index.ts';

afterEach(() => {
  vi.restoreAllMocks();
  document.body.innerHTML = '';
});

const W = 400;
const q = (sel: string): HTMLElement | null => document.querySelector<HTMLElement>(sel);
const rect = (): void => {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({ width: W, height: 72, top: 0, left: 0, right: W, bottom: 72, x: 0, y: 0, toJSON: () => ({}) } as DOMRect);
};
const pe = (type: string, x: number, t: number): void => {
  q('[data-kit="swipe"]')?.dispatchEvent(new PointerEvent(type, { bubbles: true, clientX: x, clientY: 100, pointerType: 'touch', isPrimary: true, pointerId: 1, timeStamp: t } as PointerEventInit));
};
const swipe = (from: number, to: number): void => {
  pe('pointerdown', from, 0);
  for (let i = 1; i <= 10; i++) pe('pointermove', from + ((to - from) * i) / 10, i * 50);
  pe('pointerup', to, 600);
};
const draw = (look: Look, swiped: SwipeDir[]): HTMLElement =>
  mount(() => (
    <PlatformLookProvider look={() => look}>
      {
        (
          <Swipe right={{ label: 'Mark done', icon: 'done', tone: 'good' }} left={{ label: 'Save for later', icon: 'bookmark', tone: 'accent' }} onSwipe={(d) => swiped.push(d)}>
            <ListRow title="Row" />
          </Swipe>
        ) as JSX.Element
      }
    </PlatformLookProvider>
  ));

describe('a swipe runs its action (audit A #8)', () => {
  for (const look of ['android', 'ios', 'web'] as const) {
    it(`on_${look}_a_swipe_past_the_line_runs_the_action_with_no_transitionend_and_leaves_the_row_idle`, async () => {
      rect();
      const swiped: SwipeDir[] = [];
      draw(look, swiped);
      swipe(100, 100 + W * 0.7);
      await until(() => swiped.length > 0, 2000);
      expect(swiped).toEqual(['right']);
      expect(q('[data-kit="swipe"]')?.dataset['phase'], 'not stuck at closing').toBe('idle');
    });
  }

  it('the_action_waits_for_the_rows_own_animations_then_runs', async () => {
    rect();
    const swiped: SwipeDir[] = [];
    draw('android', swiped);
    let finish: () => void = () => undefined;
    const finished = new Promise<void>((r) => (finish = r));
    const running = { playState: 'running', finished } as unknown as Animation;
    vi.spyOn(HTMLElement.prototype, 'getAnimations').mockImplementation(function (this: HTMLElement) {
      return this.getAttribute('data-kit') === 'swipe' ? [running] : [];
    });
    swipe(300, 300 - W * 0.7);
    await new Promise((r) => setTimeout(r, 100));
    expect(swiped, 'still flying').toEqual([]);
    finish();
    await until(() => swiped.length > 0, 2000);
    expect(swiped).toEqual(['left']);
  });
});
