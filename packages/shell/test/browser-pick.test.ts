// The picker's and Settings → Agents' Browser row (browser-pick.ts, ADR-0128): what each harness is offered, what's
// greyed out and why, and what's in effect.
import { describe, expect, test } from 'vitest';
import { browserRow, browserSetting, withBrowser, type BrowserCan } from '../src/frame/browser-pick.ts';

const CLAUDE: BrowserCan = { mcp: true, ownChrome: true, sandbox: false };
const CODEX: BrowserCan = { mcp: true, ownChrome: false, sandbox: false };
// Cursor and Antigravity take no per-run MCP servers (wisp-runtime `AgentStatus.browser`)
const NO_MCP: BrowserCan = { mcp: false, ownChrome: false, sandbox: false };

const greyed = (can: BrowserCan, setting: Parameters<typeof browserRow>[1]): Record<string, string | undefined> =>
  Object.fromEntries(browserRow(can, setting).choices.map((c) => [c.id, c.why]));

describe('the browser row', () => {
  test('claude_and_codex_offer_every_choice_and_show_the_one_set', () => {
    for (const can of [CLAUDE, CODEX]) {
      expect(greyed(can, 'chrome')).toEqual({ default: undefined, off: undefined, chrome: undefined, headless: undefined });
      expect(browserRow(can, 'headless').value).toBe('headless');
      expect(browserRow(can, 'chrome').note).toBeUndefined();
    }
  });

  test('cursor_and_antigravity_show_the_browser_as_unavailable_with_why_and_never_as_on', () => {
    for (const setting of ['default', 'chrome', 'headless'] as const) {
      const row = browserRow(NO_MCP, setting);
      expect(row.value).toBe('off');
      expect(row.note).toBe('noMcp');
      expect(greyed(NO_MCP, setting)).toEqual({ default: 'noMcp', off: undefined, chrome: 'noMcp', headless: 'noMcp' });
    }
  });

  test('in_a_sandbox_chrome_is_unavailable_and_falls_back_to_the_sandboxs_browser', () => {
    const row = browserRow({ ...CLAUDE, sandbox: true }, 'chrome');
    expect(row.choices.find((c) => c.id === 'chrome')?.why).toBe('sandboxClaude');
    expect(row.value).toBe('headless');
    expect(row.note).toBe('sandboxBrowser');
    expect(browserRow({ ...CODEX, sandbox: true }, 'chrome').choices.find((c) => c.id === 'chrome')?.why).toBe('sandbox');
    expect(browserRow({ ...CODEX, sandbox: true }, 'off').note).toBeUndefined();
    expect(browserRow({ ...NO_MCP, sandbox: true }, 'chrome').note).toBe('noMcp');
  });

  test('a_harnesss_own_value_wins_and_default_follows_every_harnesss_again', () => {
    expect(browserSetting('off', { codex: 'headless' }, 'codex')).toBe('headless');
    expect(browserSetting('off', { codex: 'headless' }, 'claude')).toBe('off');
    expect(browserSetting(undefined, undefined, 'claude')).toBe('default');
    expect(withBrowser({ codex: 'headless', claude: 'off' }, 'codex', 'default')).toEqual({ claude: 'off' });
    expect(withBrowser(null, 'grok', 'chrome')).toEqual({ grok: 'chrome' });
  });
});
