// R4-6: Copy turn as Markdown is one of a turn's tools: on the trail in a browser, in the answer's ⋯ menu on a phone.
import { afterEach, expect, test } from 'vitest';
import { mount, until } from './helpers.tsx';
import { GestureHostProvider, PlatformLookProvider, TurnView, type GestureHost } from '../src/index.ts';
import type { TurnTool } from '../src/app/turn-menu.ts';

afterEach(() => {
  document.body.innerHTML = '';
});

const tools = (copied: string[]): TurnTool[] => [
  { id: 'fork', icon: 'fork', label: 'Fork from this turn', act: { onClick: () => undefined } },
  { id: 'copy-md', icon: 'copy', label: 'Copy turn as Markdown', act: { onClick: () => void copied.push('md') } },
];
const turn = (look: 'web' | 'android', id: string, copied: string[]) => {
  const host = { haptic: () => undefined, actions: () => () => undefined } as unknown as GestureHost;
  return mount(() => (
    <GestureHostProvider host={host}>
      <PlatformLookProvider look={() => look}>
        <TurnView id={id} prompt="Hey" harness="Codex" time="22:31" timeTitle="22:31" acts={[]} actsTitle="" latest stats={[]} tools={tools(copied)} />
      </PlatformLookProvider>
    </GestureHostProvider>
  ));
};

test('a_browser_shows_copy_turn_as_markdown_on_the_trail', () => {
  const copied: string[] = [];
  const box = turn('web', 'w1', copied);
  const b = box.querySelector<HTMLButtonElement>('[data-kit="tb-btn"][data-a="copy-md"]');
  expect(b?.getAttribute('aria-label')).toBe('Copy turn as Markdown');
  b?.click();
  expect(copied).toEqual(['md']);
});

test('a_phone_has_it_in_the_answers_menu', async () => {
  const copied: string[] = [];
  turn('android', 'a1', copied);
  document.querySelector<HTMLElement>('[data-turn="a1"] [data-kit="tb-more"] button')?.click();
  const item = (): HTMLElement | undefined =>
    [...document.querySelectorAll<HTMLElement>('[data-kit="menu"][data-state="open"] [data-kit="menu-item"]')].find((e) => e.textContent?.includes('Copy turn as Markdown'));
  await until(() => item() !== undefined, 2000);
  item()?.click();
  await until(() => copied.length > 0, 2000);
  expect(copied).toEqual(['md']);
});
