// Audit E2/E3: the project-trust card laid out as v1's (hold.js projectCard): what it says, then what the repo would run
// (each with its file and Open file), then the buttons under them, the primary first. As a Banner's side action the
// three buttons took the width: on a phone the items were 0 px wide (a letter a line), on desktop "Hoo / k".
import { expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { KitProvider } from '@wisp/ui';
import { HeldCard } from '../src/cards/HeldCard.tsx';
import { cwdOf } from '../src/cards/model.ts';
import en from '../src/i18n/en.lazy.json' with { type: 'json' };

const t = (k: string): string => (en as Record<string, string>)[k] ?? k;

const held = {
  title: 'This repo asks to run its own programs',
  why: 'Held before it started: these would run on this computer, as you, whatever the mode.',
  start: 'Trust and start',
  without: 'Run without them',
  items: [
    { kind: 'Hook', name: 'PreToolUse (Bash)', command: 'echo hi', file: '.claude/settings.json' },
    { kind: 'MCP server', name: 'local', command: 'echo x', file: '.mcp.json' },
  ],
};

test('the_trust_card_lists_each_item_with_its_file_and_puts_the_buttons_under_them_audit_e2_e3', () => {
  const opened: string[] = [];
  const host = document.createElement('div');
  document.body.append(host);
  const dispose = render(
    () => (
      <KitProvider t={(k) => k}>
        <HeldCard held={held} t={t} onStart={() => {}} onSkip={() => {}} onWithout={() => {}} onOpen={(f) => opened.push(f)} />
      </KitProvider>
    ),
    host,
  );
  const card = host.querySelector('[data-part="turn.held"]') as HTMLElement;
  // no side column: the buttons are a row of their own under the items, Trust and start first (v1 .lm-acts)
  expect(card.querySelector('[data-kit="notice-action"]')).toBeNull();
  const acts = card.querySelector('[data-part="turn.held.actions"]') as HTMLElement;
  expect(acts.getAttribute('data-kit')).toBe('wrap');
  expect([...acts.querySelectorAll('button')].map((b) => b.textContent?.trim())).toEqual(['Trust and start', 'Run without them', 'Skip this run']);
  const items = card.querySelector('[data-part="turn.held.items"]') as HTMLElement;
  expect(items.compareDocumentPosition(acts) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  // each item names its file, and opens it
  const rows = [...card.querySelectorAll('[data-part="turn.held.item"]')];
  expect(rows.map((r) => r.querySelector('[data-part="turn.held.file"]')?.textContent)).toEqual(['.claude/settings.json', '.mcp.json']);
  (rows[1]?.querySelector('button') as HTMLButtonElement).click();
  expect(opened).toEqual(['.mcp.json']);
  dispose();
  host.remove();
});

// The card's Open file needs the task's folder: the row keeps it under `summary` (a synced row), so `task().cwd` was empty
test('the_task_folder_is_read_where_the_row_keeps_it_audit_e3', () => {
  expect(cwdOf({ summary: { cwd: '/w/proj' } })).toBe('/w/proj');
  expect(cwdOf({ cwd: '/w/top' })).toBe('/w/top');
  expect(cwdOf(undefined)).toBe('');
});
