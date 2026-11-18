// An answer's long-press on Android (QA r7, round 6 #15): the hold opens the answer's menu whatever order the browser
// sends the touch in, and the menu opens once the finger is up.
import { afterEach, describe, expect, test } from 'vitest';
import { mount, until } from './helpers.tsx';
import { GestureHostProvider, type GestureHost, PlatformLookProvider, TurnView } from '../src/index.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

// QA r7 (round 6 #15 on Android): the hold opens the answer's menu whatever order Android sends it in. QA's way: a
// touch pointerdown, 900 ms, pointerup, then the contextmenu. Android Chrome's own: pointerdown and touchstart, the
// browser takes the touch (pointercancel) as its long-press timer runs out, its contextmenu, and touchend as the finger
// lifts. The menu opens once the finger is up, and stays.
const label = (n: number): string => `Fork from answer ${n}`;
let n = 0;
const mountTurn = () => {
  n += 1;
  const name = label(n);
  const host = { haptic: () => undefined, actions: () => () => undefined } as unknown as GestureHost;
  const box = mount(() => (
    <GestureHostProvider host={host}>
      <PlatformLookProvider look={() => 'android'}>
        <TurnView id={`t${n}`} prompt="Hey" harness="Codex" time="22:31" timeTitle="22:31" acts={[]} actsTitle="" latest stats={[]} tools={[{ id: 'fork', icon: 'fork', label: name, act: { onClick: () => undefined } }]} />
      </PlatformLookProvider>
    </GestureHostProvider>
  ));
  const ask = box.querySelector('[data-kit="turn"] [data-kit="ask"]') as HTMLElement;
  const open = (): boolean => [...document.querySelectorAll('[data-kit="menu-item"]')].some((e) => e.textContent?.includes(name) && e.closest('[data-kit="menu"]')?.getAttribute('data-state') === 'open');
  return { ask, open };
};
const pe = (el: Element, type: string): boolean => el.dispatchEvent(new PointerEvent(type, { bubbles: true, cancelable: true, pointerType: 'touch', isPrimary: true, pointerId: 1, clientX: 120, clientY: 80, buttons: type === 'pointerup' ? 0 : 1 }));
const touch = (el: Element, type: string): boolean => el.dispatchEvent(new Event(type, { bubbles: true, cancelable: true }));
const wait = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

describe('an answer’s long-press on Android (QA r7)', () => {
  test('qa_s_way_pointerdown_900ms_pointerup_then_contextmenu_opens_the_menu', async () => {
    const { ask, open } = mountTurn();
    pe(ask, 'pointerdown');
    await wait(900);
    pe(ask, 'pointerup');
    ask.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 120, clientY: 80 }));
    await until(open, 2000);
    await wait(200);
    expect(open(), 'still open').toBe(true);
  });

  test('android_chromes_way_pointercancel_before_its_contextmenu_then_touchend_opens_the_menu', async () => {
    const { ask, open } = mountTurn();
    pe(ask, 'pointerdown');
    touch(ask, 'touchstart');
    await wait(300);
    // the browser takes the touch for its own long-press: the pointer stream ends, the finger is still down
    pe(ask, 'pointercancel');
    await wait(200);
    const cm = new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 120, clientY: 80 });
    ask.dispatchEvent(cm);
    expect(cm.defaultPrevented, 'the system menu is not shown').toBe(true);
    await wait(100);
    expect(open(), 'not under the finger yet').toBe(false);
    touch(ask, 'touchend');
    await until(open, 2000);
    await wait(200);
    expect(open(), 'still open').toBe(true);
  });

  test('a_contextmenu_long_after_a_press_or_after_a_scroll_is_not_the_hold', async () => {
    const { ask, open } = mountTurn();
    pe(ask, 'pointerdown');
    // the finger moved (a scroll): the browser cancels the pointer
    ask.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, pointerType: 'touch', isPrimary: true, pointerId: 1, clientX: 120, clientY: 160 }));
    pe(ask, 'pointercancel');
    touch(ask, 'touchend');
    ask.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 120, clientY: 160 }));
    await wait(300);
    expect(open()).toBe(false);
  });
});
