// UX 2026-11-18 desktop #8 on the task page: the reply box's harness chip carries the same usage ring as Home's (the
// reply's harness past 80% of a plan window), drawing only; its words are a line of the chip's title and accessible name.
import { expect, test } from 'vitest';
import { mount } from './helpers.tsx';
import { ReplyBox } from '../src/app/Task.tsx';

const box = (usage?: { pct: number; tone: 'warn' | 'bad'; title: string }) => (
  <ReplyBox placeholder="Follow up" value="" onInput={() => {}} onSend={() => {}} attach="Attach" mode={{ label: 'Auto', title: 'Mode' }} runner={{ title: 'Claude Code', logo: '/claude.svg', ...(usage && { usage }) }} team="Team" goal="Goal" later="Later" send="Send" />
);

test('the_reply_chip_rings_its_logo_past_80_percent', () => {
  const host = mount(() => box({ pct: 97, tone: 'bad', title: 'Weekly 97% used' }));
  const chip = host.querySelector('[data-part~="composer.harness"]');
  const ring = chip?.querySelector<HTMLElement>('[data-kit="rb-logo"] [data-kit="rb-ring"]');
  expect(ring?.dataset['tone']).toBe('bad');
  expect(ring?.dataset['part']).toBe('composer.harness.usage');
  expect(ring?.getAttribute('aria-hidden')).toBe('true');
  expect(ring?.style.getPropertyValue('--ctx')).toBe('97%');
  expect(chip?.getAttribute('title')).toBe('Claude Code\nWeekly 97% used');
  expect(chip?.getAttribute('aria-label')).toBe('Claude Code\nWeekly 97% used');
});

test('no_ring_and_the_chip_unchanged_under_80_percent', () => {
  const host = mount(() => box());
  const chip = host.querySelector('[data-part~="composer.harness"]');
  expect(chip?.querySelector('[data-kit="rb-ring"]')).toBeNull();
  expect(chip?.getAttribute('title')).toBe('Claude Code');
});
