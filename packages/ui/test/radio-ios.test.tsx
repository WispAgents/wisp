// QA r14 #3: iOS has no radio button. A choice list is rows, the chosen one with a ✓ at its end (HIG: a table's
// checkmark): on iOS each option marks itself chosen and carries its ✓; the radio itself stays for keyboards and
// screen readers. The browser's rings are as they were (no ✓ in its markup).
import { afterEach, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { mount } from './helpers.tsx';
import { PlatformLookProvider, RadioGroup, type Look } from '../src/index.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

const options = [
  { value: 'link', label: 'Anyone with the link', hint: 'Reads it in a browser.' },
  { value: 'pass', label: 'Only with a passkey', hint: 'Opening it asks for the passkey.' },
];

const draw = (look: Look) => {
  const [v, setV] = createSignal('link');
  const onChange = vi.fn((x: string) => setV(x));
  const box = mount(() => (
    <PlatformLookProvider look={() => look}>
      <RadioGroup label="Who can open it" value={v()} onChange={onChange} options={options} />
    </PlatformLookProvider>
  ));
  const rows = (): HTMLElement[] => [...box.querySelectorAll<HTMLElement>('[data-kit="radio"]')];
  return { box, rows, onChange };
};

test('on_ios_each_option_is_a_row_the_chosen_one_marked_with_its_check_at_the_end', () => {
  const { rows, onChange } = draw('ios');
  expect(rows().map((r) => r.hasAttribute('data-checked'))).toEqual([true, false]);
  for (const r of rows()) expect(r.lastElementChild?.getAttribute('data-kit'), 'the ✓ ends the row').toBe('radio-check');
  // the radio is still there (a keyboard, a screen reader), and a tap on the row picks it
  (rows()[1]?.querySelector('input[type="radio"]') as HTMLInputElement).click();
  expect(onChange).toHaveBeenCalledWith('pass');
  expect(rows().map((r) => r.hasAttribute('data-checked'))).toEqual([false, true]);
});

test('the_browser_keeps_its_radio_rings_no_check_in_the_markup', () => {
  const { box } = draw('web');
  expect(box.querySelector('[data-kit="radio-check"]')).toBeNull();
});
