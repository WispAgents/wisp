// QA round 1 #13: Home's usage row (wisp.budget, loaded after the first screen) arrived 230–310 ms after load and pushed
// Needs you down 35 px. Reserved holds the room its content took last time, and says when that changes, so the row
// lands in the room already made for it.
import { expect, test } from 'vitest';
import { createRoot } from 'solid-js';
import { Reserved } from '../src/index.ts';
import { mount } from './helpers.tsx';

test('reserved_holds_the_last_height_it_was_given', () => {
  const host = mount(() => (
    <Reserved height={37} part="home.usage">
      <span />
    </Reserved>
  ));
  const el = host.querySelector<HTMLElement>('[data-kit="reserved"]');
  expect(el?.style.getPropertyValue('--reserve-h')).toBe('37px');
  expect(el?.getAttribute('data-part')).toBe('home.usage');
});

test('reserved_with_nothing_kept_holds_no_room', () => {
  const host = mount(() => <Reserved height={0}>{null}</Reserved>);
  expect(host.querySelector<HTMLElement>('[data-kit="reserved"]')?.style.getPropertyValue('--reserve-h')).toBe('');
});

// review of #13: a row that stopped showing (its plugin off, no plan any more) left the room it took once, empty,
// for good. Gone without showing while held, the room is let go: the next time nothing is held.
test('reserved_lets_its_room_go_when_its_content_never_showed', () => {
  const said: number[] = [];
  let stop: (() => void) | undefined;
  createRoot((dispose) => {
    stop = dispose;
    return (
      <Reserved height={37} onHeight={(px) => void said.push(px)}>
        {null}
      </Reserved>
    );
  });
  stop?.();
  expect(said).toEqual([0]);
});
