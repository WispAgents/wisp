// Owner 2026-11-21 ("the dropdown and menu in the composer page are not consistent in the mobile app; the best one is
// Delegate, use the same for all"): in the phone apps every menu of the composer (Mode, Runs on, More, Start later)
// opens as the bottom sheet Delegate's panel opens as: its handle, the menu's own title and ✕; never the platform's
// anchored dropdown. iOS draws it too (a button's menu is not a long-press's context menu). The browser keeps its rule.
import { expect, test } from 'vitest';
import { mount, until } from './helpers.tsx';
import { PlatformLookProvider, ReplyBox, type Look } from '../src/index.ts';
import type { MenuEntry } from '../src/controls/Menu.tsx';

const MODES: MenuEntry[] = [
  { value: 'plan', label: 'Plan', hint: 'Reads and plans, changes nothing' },
  { value: 'auto', label: 'Full auto', hint: 'No permission prompts at all', checked: true },
];

function box(look: Look, picked: string[]): HTMLElement {
  return mount(() => (
    <PlatformLookProvider look={() => look}>
      <ReplyBox
        placeholder="Follow up…"
        value=""
        onInput={() => undefined}
        onSend={() => undefined}
        attach="Attach files"
        mode={{ label: 'Full auto', title: 'Full auto: no permission prompts at all', act: { menu: { title: 'Mode', items: MODES, onSelect: (v) => void picked.push(v) } } }}
        team="Delegate"
        goal="Add a goal"
        later="Start later"
        send="Send"
      />
    </PlatformLookProvider>
  ));
}

for (const look of ['android', 'ios'] as const) {
  test(`a_composer_menu_opens_as_the_titled_sheet_on_${look}`, async () => {
    const picked: string[] = [];
    box(look, picked);
    document.querySelector<HTMLButtonElement>('[data-part="composer.mode"]')?.click();
    await until(() => document.querySelector('[data-kit="sheet-menu"]') !== null);
    const sheet = document.querySelector('[data-kit="sheet"]');
    expect(sheet, 'a bottom sheet, as Delegate').not.toBeNull();
    expect(sheet?.querySelector('[data-kit="dialog-title"]')?.textContent, "the menu's own title, not the button's value").toBe('Mode');
    expect(document.querySelector('[role="menu"]:not([data-kit="sheet-menu"])'), 'no anchored dropdown').toBeNull();
    [...document.querySelectorAll<HTMLButtonElement>('[data-kit="sheet-item"]')].find((b) => b.textContent?.includes('Plan'))?.click();
    await until(() => document.querySelector('[data-kit="sheet-menu"]') === null);
    expect(picked).toEqual(['plan']);
  });
}
