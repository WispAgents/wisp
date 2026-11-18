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

// QA r8 #2: the hold's menu opens where the finger was, kept on screen, never at the answer's ⋯ (in its footer, often
// below the fold: the menu opened off screen and nothing seemed to happen). The ⋯ still opens it at the ⋯.
describe('the long-press menu opens at the finger (QA r8 #2)', () => {
  const positionerOf = (name: string): HTMLElement | undefined =>
    [...document.querySelectorAll<HTMLElement>('[data-kit="menu"]')].find((m) => m.textContent?.includes(name) && m.getAttribute('data-state') === 'open')?.parentElement ?? undefined;
  /** the positioner's place as Floating UI set it (Zag's --x, --y) */
  const placeOf = (el: HTMLElement): { x: number; y: number } => ({ x: Number.parseFloat(el.style.getPropertyValue('--x')), y: Number.parseFloat(el.style.getPropertyValue('--y')) });
  const placed = (name: string): boolean => (positionerOf(name)?.style.getPropertyValue('--y') ?? '') !== '';

  test('qa_s_long_press_opens_the_menu_at_the_pressed_point_not_at_the_footers_dots', async () => {
    const { ask, open } = mountTurn();
    // the answer's ⋯ sits far below (its footer under the fold)
    const dots = document.querySelector<HTMLElement>(`[data-turn="t${n}"] [data-kit="tb-more"] button`) as HTMLElement;
    dots.getBoundingClientRect = () => ({ left: 330, right: 362, top: 1141, bottom: 1173, width: 32, height: 32, x: 330, y: 1141, toJSON: () => ({}) }) as DOMRect;
    pe(ask, 'pointerdown');
    // Android's long touch selects the word under the finger
    getSelection()?.selectAllChildren(ask);
    await wait(900);
    pe(ask, 'pointerup');
    ask.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 120, clientY: 80 }));
    await until(open, 2000);
    await until(() => placed(label(n)), 2000);
    const at = placeOf(positionerOf(label(n)) as HTMLElement);
    // the text the browser selected under the hold is let go (its own bubble, "Comment", gave way to the menu)
    expect(getSelection()?.toString() ?? '').toBe('');
    // beside the finger (120, 80), nowhere near the ⋯ at y 1141
    expect(Math.abs(at.y - 80), `y ${at.y}`).toBeLessThan(60);
    expect(at.y).toBeLessThan(1000);
  });

  test('the_dots_still_open_it_at_the_dots', async () => {
    const { open } = mountTurn();
    const dots = document.querySelector<HTMLElement>(`[data-turn="t${n}"] [data-kit="tb-more"] button`) as HTMLElement;
    dots.getBoundingClientRect = () => ({ left: 330, right: 362, top: 600, bottom: 632, width: 32, height: 32, x: 330, y: 600, toJSON: () => ({}) }) as DOMRect;
    dots.click();
    await until(open, 2000);
    await until(() => placed(label(n)), 2000);
    const at = placeOf(positionerOf(label(n)) as HTMLElement);
    expect(Math.abs(at.y - 600), `y ${at.y}`).toBeLessThan(60);
  });
});

// QA r9 #2 (UX 2026-11-18 phone #8): the answer's menu holds what the answer offers: Copy, Comment… (on the block held),
// its trail's tools (Agent notes, Restore, Fork) and Good / Bad result; the ⋯ the same, without Comment… (no block held)
const mountFull = (onCopy: (text: string) => void, commented: () => void, rated: (r: string) => void) => {
  n += 1;
  const host = { haptic: () => undefined, actions: () => () => undefined } as unknown as GestureHost;
  const box = mount(() => (
    <GestureHostProvider host={host}>
      <PlatformLookProvider look={() => 'android'}>
        <TurnView
          id={`t${n}`}
          prompt="Hey"
          harness="Codex"
          time="22:31"
          timeTitle="22:31"
          acts={[]}
          actsTitle=""
          stats={[]}
          answer={
            <div data-kit="answer">
              <div data-kit="answer-block">
                <p>I committed the docs/dev.md sentence.</p>
                <button type="button" data-kit="cmt-add" onClick={commented}>
                  c
                </button>
              </div>
            </div>
          }
          tools={[
            { id: 'notes', icon: 'text', label: 'Agent notes', act: { onClick: () => undefined } },
            { id: 'fork', icon: 'fork', label: `Fork ${n}`, act: { onClick: () => undefined } },
          ]}
          phoneMenu={{
            copy: { label: 'Copy', onCopy },
            comment: 'Comment…',
            more: [
              { id: 'good', icon: 'thumb-up', label: 'Good result', on: false, act: { onClick: () => rated('up') } },
              { id: 'bad', icon: 'thumb-down', label: 'Bad result', on: false, act: { onClick: () => rated('down') } },
            ],
          }}
        />
      </PlatformLookProvider>
    </GestureHostProvider>
  ));
  const rows = (): HTMLElement[] =>
    [...document.querySelectorAll<HTMLElement>('[data-kit="menu"][data-state="open"]')].filter((m) => m.textContent?.includes(`Fork ${n}`)).flatMap((m) => [...m.querySelectorAll<HTMLElement>('[data-kit="menu-item"]')]);
  return { box, rows, words: () => rows().map((r) => r.textContent?.trim() ?? '') };
};

describe('the answer menu holds the answer’s actions (QA r9 #2)', () => {
  test('a_hold_on_a_block_offers_copy_comment_the_tools_and_good_bad_in_that_order', async () => {
    const copied: string[] = [];
    let comments = 0;
    const rated: string[] = [];
    const { box, rows, words } = mountFull((t) => copied.push(t), () => (comments += 1), (r) => rated.push(r));
    const p = box.querySelector('[data-kit="answer-block"] p') as HTMLElement;
    p.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true, pointerType: 'touch', isPrimary: true, pointerId: 1, clientX: 120, clientY: 80 }));
    await wait(900);
    p.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, cancelable: true, pointerType: 'touch', isPrimary: true, pointerId: 1, clientX: 120, clientY: 80 }));
    await until(() => rows().length > 0, 2000);
    expect(words()).toEqual(['Copy', 'Comment…', 'Agent notes', `Fork ${n}`, 'Good result', 'Bad result']);
    rows()[0]?.click();
    await until(() => copied.length > 0);
    expect(copied[0]).toContain('I committed the docs/dev.md sentence.');
    expect(copied[0], 'not the Comment button’s own text').not.toMatch(/c$/);
  });

  test('comment_opens_the_held_blocks_comment_and_good_rates', async () => {
    let comments = 0;
    const rated: string[] = [];
    const { box, rows } = mountFull(() => undefined, () => (comments += 1), (r) => rated.push(r));
    const hold = async (): Promise<void> => {
      const p = box.querySelector('[data-kit="answer-block"] p') as HTMLElement;
      p.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, cancelable: true, pointerType: 'touch', isPrimary: true, pointerId: 1, clientX: 120, clientY: 80 }));
      await wait(900);
      p.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, cancelable: true, pointerType: 'touch', isPrimary: true, pointerId: 1, clientX: 120, clientY: 80 }));
      await until(() => rows().length > 0, 2000);
    };
    await hold();
    // read, then tapped (a person's tap comes well after the hold's lift)
    await wait(400);
    rows()[1]?.click();
    await until(() => comments === 1, 2000);
    expect(comments, 'the held block’s own Comment').toBe(1);
    await until(() => rows().length === 0, 2000);
    expect(rows(), 'the menu closed').toEqual([]);
    await hold();
    rows().find((r) => r.textContent?.includes('Good result'))?.click();
    await until(() => rated.length > 0);
    expect(rated).toEqual(['up']);
  }, 15_000);

  test('the_dots_offer_the_same_without_comment', async () => {
    const { box, words, rows } = mountFull(() => undefined, () => undefined, () => undefined);
    (box.querySelector('[data-kit="tb-more"] button') as HTMLElement).click();
    await until(() => rows().length > 0, 2000);
    expect(words()).toEqual(['Copy', 'Agent notes', `Fork ${n}`, 'Good result', 'Bad result']);
  });
});

// QA r10 #6: the answer's ⋯ ends its action row (Agent notes, Restore, Fork, 👍, 👎, ⋯), never a line of its own; an
// older answer, its icons folded into the ⋯, keeps the ⋯ in that row
describe('the ⋯ ends the action row (QA r10 #6)', () => {
  for (const latest of [true, false]) {
    test(`on_a_phone_the_dots_are_the_last_of_the_${latest ? 'latest' : 'older'}_answers_action_row`, () => {
      n += 1;
      const host = { haptic: () => undefined, actions: () => () => undefined } as unknown as GestureHost;
      const box = mount(() => (
        <GestureHostProvider host={host}>
          <PlatformLookProvider look={() => 'ios'}>
            <TurnView id={`t${n}`} prompt="Hey" harness="Codex" time="22:31" timeTitle="22:31" acts={[]} actsTitle="" stats={[]} latest={latest} rate={<span data-kit="rate" />} tools={[{ id: 'fork', icon: 'fork', label: 'Fork', act: { onClick: () => undefined } }]} />
          </PlatformLookProvider>
        </GestureHostProvider>
      ));
      const row = box.querySelector('[data-kit="trail-bar"] > [data-kit="tb-tools"]') as HTMLElement;
      expect(row.lastElementChild?.getAttribute('data-kit')).toBe('tb-more');
      expect(box.querySelectorAll('[data-kit="trail-bar"] > [data-kit="tb-more"]').length, 'not a cell of its own').toBe(0);
    });
  }
  test('in_the_browser_there_is_no_dots_and_the_row_is_as_before', () => {
    const box = mount(() => <TurnView id="tw" prompt="Hey" harness="Codex" time="22:31" timeTitle="22:31" acts={[]} actsTitle="" stats={[]} latest tools={[{ id: 'fork', icon: 'fork', label: 'Fork', act: { onClick: () => undefined } }]} />);
    expect(box.querySelector('[data-kit="tb-more"]')).toBeNull();
    expect(box.querySelector('[data-kit="tb-tools"]')?.lastElementChild?.getAttribute('data-kit')).toBe('tb-btn');
  });
});

// QA r10 #5: the answer's menu is the one overlay: while it is open the page is marked, and the phone looks hide the
// selection's Comment bubble and a block's Comment button under it (two overlays at once on Android)
describe('the answer menu is the one overlay (QA r10 #5)', () => {
  test('while_the_answer_menu_is_open_the_page_is_marked_and_the_mark_goes_with_it', async () => {
    const { ask, open } = mountTurn();
    pe(ask, 'pointerdown');
    await wait(900);
    pe(ask, 'pointerup');
    await until(open, 2000);
    expect(open()).toBe(true);
    expect(document.body.hasAttribute('data-answer-menu')).toBe(true);
    document.querySelector('[data-kit="menu"][data-state="open"]')?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await until(() => !open(), 2000);
    expect(open()).toBe(false);
    expect(document.body.hasAttribute('data-answer-menu')).toBe(false);
  });

  for (const look of ['android', 'ios'] as const) {
    test(`the_${look}_look_hides_the_comment_bubble_and_buttons_under_the_answer_menu`, () => {
      const css = Object.entries(import.meta.glob<string>('../src/css/platform/*.css', { query: '?raw', import: 'default', eager: true })).find(([p]) => p.endsWith(`/${look}.css`))?.[1] ?? '';
      expect(css).toMatch(new RegExp(`:root\\[data-platform='${look}'\\] \\[data-answer-menu\\] \\[data-kit='cmt-bubble'\\],\\s*:root\\[data-platform='${look}'\\] \\[data-answer-menu\\] \\[data-kit='cmt-add'\\]:not\\(\\[data-on\\]\\) \\{ visibility: hidden; \\}`));
    });
  }
});
