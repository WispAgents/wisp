// The side panel's tabs as qa/parity.mjs opens them (docs/ux/2026-11-18-desktop.md #5): the strip holds the tabs with
// something in them, so v1's Preview tab (qa/views.mjs task.preview) is one press while the task's app runs and two
// otherwise: the strip's + (More tabs), then Preview in its menu. Pure: it takes the page it presses on.

export const PREVIEW_TAB = '[data-part="panel.tabs"] [data-kit="tab"][data-value="preview"]';
export const TABS_MORE = '[data-part~="panel.tabs.more.trigger"]';
export const PREVIEW_ITEM = '[data-kit="menu-item"][data-value="preview"]';

interface Page {
  locator(sel: string): { count(): Promise<number>; first(): { click(o?: { force?: boolean }): Promise<unknown> } };
  waitForTimeout(ms: number): Promise<unknown>;
}

/** Open the side panel's Preview, from the strip or from behind its +. */
export async function openPreview(p: Page): Promise<void> {
  if (await p.locator(PREVIEW_TAB).count()) {
    await p.locator(PREVIEW_TAB).first().click({ force: true });
    return;
  }
  await p.locator(TABS_MORE).first().click({ force: true });
  await p.waitForTimeout(300);
  await p.locator(PREVIEW_ITEM).first().click({ force: true });
}
