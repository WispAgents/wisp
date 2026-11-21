// audit C tasks #2: after + › Start later › a time, the caret stayed on `+`, so Enter opened the menu again instead of
// sending. A choice inside a row's own menu hands the caret back to the box (after the menu gave it to `+`); a row of
// its own (Attach, Goal) opens what it opens and is left alone.
import { expect, test, vi } from 'vitest';
import { plusTool } from '../src/frame/composer-plus.ts';

test('a_choice_in_a_rows_own_menu_gives_the_caret_back_to_the_box_audit_c_2', async () => {
  const refocus = vi.fn();
  const picked = vi.fn();
  const attach = vi.fn();
  const tool = plusTool({
    part: 'composer.plus',
    label: '+',
    title: '+',
    refocus,
    sections: [
      [{ id: 'attach', part: 'composer.attach', icon: 'clip', label: 'Attach', act: { onClick: attach } }],
      [{ id: 'later', part: 'composer.sendLater', icon: 'clock', label: 'Start later', act: { menu: { items: [{ value: '10m', label: 'In 10 min' }], onSelect: picked } } }],
    ],
  });
  const onSelect = tool.act?.menu?.onSelect as (v: string) => void;
  onSelect('10m');
  expect(picked).toHaveBeenCalledWith('10m');
  expect(refocus, 'not before the menu has closed').not.toHaveBeenCalled();
  await new Promise((r) => setTimeout(r, 0));
  expect(refocus).toHaveBeenCalledTimes(1);
  onSelect('attach');
  await new Promise((r) => setTimeout(r, 0));
  expect(attach).toHaveBeenCalled();
  expect(refocus).toHaveBeenCalledTimes(1);
});
