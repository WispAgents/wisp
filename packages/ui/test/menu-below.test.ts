// QA desktop r3 #5: Delegate's Presets menu lay over the panel's intro when it opened under its button (r2 #3), and over
// the launcher's bar and Send when it opened above. `below`: it opens under the panel's head instead, at the
// button's side, inside the panel.
import { expect, test } from 'vitest';
import { underBox } from '../src/controls/menu-anchor.ts';

const box = (el: HTMLElement, r: { x: number; y: number; width: number; height: number }): void => {
  el.getBoundingClientRect = () => ({ ...r, left: r.x, top: r.y, right: r.x + r.width, bottom: r.y + r.height, toJSON: () => r }) as DOMRect;
};

test('the anchor is the trigger stretched to the bottom of its box, so the menu opens under the box_qa_desktop_r3_5', () => {
  const head = document.createElement('header');
  head.setAttribute('data-kit', 'tp-head');
  const button = document.createElement('button');
  head.append(button);
  document.body.append(head);
  box(head, { x: 700, y: 330, width: 540, height: 64 });
  box(button, { x: 1160, y: 334, width: 80, height: 24 });
  // the trigger's side, from its top to the head's bottom (394): bottom-end places the menu under it
  expect(underBox("[data-kit='tp-head']")(button)).toEqual({ x: 1160, y: 334, width: 80, height: 60 });
  // no such box around it: the trigger itself
  expect(underBox("[data-kit='nope']")(button)).toEqual({ x: 1160, y: 334, width: 80, height: 24 });
  // not an element (a virtual anchor): Zag's own
  expect(underBox("[data-kit='tp-head']")(null)).toBeNull();
  head.remove();
});
