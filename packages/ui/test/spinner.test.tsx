// The kit's spinner always says what it's doing in its tooltip, and after a while that it's still waiting (TRY-12:
// a spinner with no tooltip, spinning on, as a command waits on a computer that's away).
import { afterEach, expect, test, vi } from 'vitest';
import { mount } from './helpers.tsx';
import { SLOW_MS, Spinner } from '../src/controls/Spinner.tsx';
import { Button } from '../src/controls/Button.tsx';
import { ActIcon } from '../src/app/act.tsx';

afterEach(() => vi.useRealTimers());

test('a spinner has a tooltip, and says it is still waiting once it has spun a while', async () => {
  vi.useFakeTimers();
  const host = mount(() => <Spinner label="Saving" />);
  const s = host.querySelector<HTMLElement>('[data-kit="spinner"]');
  expect([s?.title, s?.getAttribute('aria-label')]).toEqual(['Saving', 'Saving']);
  vi.advanceTimersByTime(SLOW_MS + 1);
  await Promise.resolve();
  expect(s?.title).toMatch(/^Still waiting/);
  expect(s?.hasAttribute('data-slow')).toBe(true);
});

test("a loading Button's and a pending control's spinners are the same, each with its tooltip", () => {
  const host = mount(() => (
    <div>
      <Button loading>Ship it</Button>
      <ActIcon act={{ pending: true }}>
        <span>icon</span>
      </ActIcon>
    </div>
  ));
  const titles = [...host.querySelectorAll<HTMLElement>('[data-kit="spinner"]')].map((s) => s.title);
  expect(titles).toEqual(['Loading', 'Loading']);
});
