// Starts when (owner 2026-11-28, design B): the trigger form's kinds behind one field, in groups under a search box.
import { beforeAll, describe, expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { frame, mount, tick } from './helpers.tsx';
import { TriggerKinds, matchKinds, type TriggerKindGroup } from '../src/app/Automations.tsx';
import { preloadControls } from '../src/controls/lazy-part.ts';

beforeAll(() => preloadControls());

const GROUPS: TriggerKindGroup[] = [
  { id: 'time', label: 'On a schedule', kinds: [{ key: 'schedule', label: 'Schedule', hint: 'At times you pick', icon: 'clock' }] },
  {
    id: 'github',
    label: 'On GitHub',
    kinds: [
      { key: 'ci', label: 'CI failed', hint: 'A GitHub Actions run fails', icon: 'warn' },
      { key: 'review', label: 'Review requested', hint: 'Someone is asked to review a PR', icon: 'merge' },
    ],
  },
  { id: 'app', label: 'From an app', kinds: [{ key: 'webhook:sentry', label: 'Sentry alert', hint: 'A Sentry webhook', icon: 'warn' }] },
];

const key = (el: Element, k: string): void => void el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true }));
const type = (el: HTMLInputElement, v: string): void => {
  el.value = v;
  el.dispatchEvent(new InputEvent('input', { bubbles: true }));
};
const field = (): HTMLButtonElement => document.querySelector('[data-kit="au-kind"]') as HTMLButtonElement;
const box = (): HTMLInputElement | null => document.querySelector('[data-part="automations.kinds.filter"] input');
const rows = (): string[] => [...document.querySelectorAll<HTMLElement>('[data-kit="au-kind-list"] [role="option"]')].map((r) => r.dataset['kind'] ?? '');

async function opened(): Promise<void> {
  field().click();
  await tick(30);
  await frame();
}

describe('TriggerKinds', () => {
  test('matchKinds keeps the kinds every word finds in a label, hint or group, and drops empty groups', () => {
    expect(matchKinds(GROUPS, '').map((g) => g.id)).toEqual(['time', 'github', 'app']);
    expect(matchKinds(GROUPS, 'github').flatMap((g) => g.kinds.map((k) => k.key))).toEqual(['ci', 'review']);
    expect(matchKinds(GROUPS, 'review PR').flatMap((g) => g.kinds.map((k) => k.key))).toEqual(['review']);
    expect(matchKinds(GROUPS, 'nothing here')).toEqual([]);
  });

  test('the field shows the picked kind; it opens grouped, the pick selected', async () => {
    mount(() => <TriggerKinds groups={GROUPS} value="ci" onPick={() => undefined} search="Search triggers…" empty="No trigger matches" />);
    expect(field().textContent).toContain('CI failed');
    expect(field().textContent).toContain('A GitHub Actions run fails');
    await opened();
    expect(rows()).toEqual(['schedule', 'ci', 'review', 'webhook:sentry']);
    expect([...document.querySelectorAll('[data-kit="au-kind-group"] [data-kit="menu-group-label"]')].map((h) => h.textContent)).toEqual(['On a schedule', 'On GitHub', 'From an app']);
    expect(document.querySelector('[role="option"][aria-selected="true"]')?.getAttribute('data-kind')).toBe('ci');
  });

  test('typing narrows, ↓ moves, Enter picks and closes', async () => {
    const [v, setV] = createSignal('schedule');
    mount(() => <TriggerKinds groups={GROUPS} value={v()} onPick={setV} search="Search triggers…" empty="No trigger matches" />);
    await opened();
    const input = box() as HTMLInputElement;
    type(input, 'warn');
    expect(rows()).toEqual([]);
    expect(document.querySelector('[data-kit="au-kind-empty"]')?.textContent).toBe('No trigger matches');
    type(input, 'a');
    expect(rows()).toEqual(['schedule', 'ci', 'review', 'webhook:sentry']);
    type(input, 'fails');
    expect(rows()).toEqual(['ci']);
    type(input, 'e');
    key(input, 'ArrowDown');
    key(input, 'ArrowDown');
    const lit = document.querySelector('[role="option"][data-highlighted]')?.getAttribute('data-kind');
    expect(input.getAttribute('aria-activedescendant')).toBe(document.querySelector('[role="option"][data-highlighted]')?.id);
    key(input, 'Enter');
    await tick(30);
    expect(v()).toBe(lit);
    expect(field().getAttribute('aria-expanded')).toBe('false');
  });

  test('a click on a row picks it', async () => {
    const [v, setV] = createSignal('schedule');
    mount(() => <TriggerKinds groups={GROUPS} value={v()} onPick={setV} search="Search triggers…" empty="No trigger matches" />);
    await opened();
    (document.querySelector('[role="option"][data-kind="webhook:sentry"]') as HTMLElement).click();
    await tick(30);
    expect(v()).toBe('webhook:sentry');
    expect(field().textContent).toContain('Sentry alert');
  });
});
