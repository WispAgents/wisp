// audit-B #1: a long-press opens the phone menu under the finger (Android's sheet, iOS's context menu); lifting that
// finger must not run the item it lands on (iOS context menu, Material sheet, v1). An item runs only on a new tap; a
// click no finger made (a key, a screen reader) still runs it.
import { afterEach, expect, test, vi } from 'vitest';
import { mount } from './helpers.tsx';
import { PhoneMenu } from '../src/controls/phone.tsx';

afterEach(() => {
  document.body.innerHTML = '';
});

const ITEMS = [
  { value: 'archive', label: 'Archive' },
  { value: 'pin', label: 'Pin' },
];

function open(look: 'ios' | 'android') {
  const row = document.createElement('div');
  row.setAttribute('data-kit', 'list-row');
  row.textContent = 'Filler 9';
  document.body.append(row);
  const onSelect = vi.fn();
  const onClose = vi.fn();
  mount(() => <PhoneMenu look={look} items={ITEMS} onSelect={onSelect} onClose={onClose} target={row} x={40} part="row.menu" />);
  const item = (): HTMLElement => {
    const el = [...document.querySelectorAll<HTMLElement>("[role='menuitem']")].find((b) => b.textContent?.includes('Archive'));
    if (!el) throw new Error('no Archive item');
    return el;
  };
  return { onSelect, onClose, item };
}

const lift = (el: HTMLElement): void => {
  el.dispatchEvent(new PointerEvent('pointerup', { bubbles: true, pointerType: 'touch', isPrimary: true }));
  el.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, detail: 1 }));
};
const tap = (el: HTMLElement): void => {
  el.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, pointerType: 'touch', isPrimary: true }));
  lift(el);
};

for (const look of ['ios', 'android'] as const) {
  test(`on_${look}_lifting_the_long_press_finger_on_an_item_runs_nothing_and_the_menu_stays`, () => {
    const m = open(look);
    lift(m.item());
    expect(m.onSelect).not.toHaveBeenCalled();
    expect(m.onClose).not.toHaveBeenCalled();
    // a new tap runs it
    tap(m.item());
    expect(m.onSelect).toHaveBeenCalledWith('archive');
  });

  test(`on_${look}_a_click_no_finger_made_runs_the_item_at_once`, () => {
    const m = open(look);
    m.item().dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, detail: 0 }));
    expect(m.onSelect).toHaveBeenCalledWith('archive');
  });
}
