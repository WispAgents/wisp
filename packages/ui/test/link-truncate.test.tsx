// QA round 2 #14: a computer's grid address ("this-computer--m0xoo.grid.trywisp.dev") broke over three lines in a
// phone's Machines sheet. A Link can be one line, cut with an ellipsis where it doesn't fit.
import { expect, test } from 'vitest';
import { Link } from '../src/index.ts';
import { mount } from './helpers.tsx';

test('a_truncated_link_says_so_and_a_plain_one_does_not', () => {
  const host = mount(() => (
    <>
      <Link href="https://a.example" truncate title="a.example" part="x.one">
        a.example
      </Link>
      <Link href="https://b.example" part="x.two">
        b.example
      </Link>
    </>
  ));
  expect(host.querySelector('[data-part="x.one"]')?.hasAttribute('data-truncate')).toBe(true);
  expect(host.querySelector('[data-part="x.two"]')?.hasAttribute('data-truncate')).toBe(false);
});
