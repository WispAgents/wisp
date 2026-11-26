// Connectors audit r3: an SSE server pasted came in as streamable HTTP; a connector a harness keeps its own server for
// said nothing of it on its row; a refresh that failed for now read "Needs sign-in"; and "Not now" hid servers with no
// way back to them.
import { afterEach, expect, test } from 'vitest';
import { render } from 'solid-js/web';
import { bodyOf, pasteDrafts, statusOf } from '../src/lib.ts';
import { RowNotes } from '../src/panes/Connectors.tsx';
import { Review } from '../src/panes/Review.tsx';
import type { ConnectorView, FoundServer } from '../src/types.ts';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});

function mount(view: () => ReturnType<typeof RowNotes>): HTMLElement {
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(view, host);
  return host;
}

test('a_pasted_sse_server_stays_sse', () => {
  const r = pasteDrafts(JSON.stringify({ mcpServers: { old: { type: 'sse', url: 'https://mcp.example.com/sse' }, new: { url: 'https://mcp.example.com/mcp' } } }), '');
  expect(r?.draft.sse).toBe(true);
  expect(bodyOf(r!.draft).transport).toBe('sse');
  expect(r?.more[0]?.sse).toBeUndefined();
  expect(bodyOf(r!.more[0]!).transport).toBe('remote');
  expect(pasteDrafts(JSON.stringify({ transport: 'SSE', url: 'https://a.example/sse' }), 'a')?.draft.sse).toBe(true);
});

test('a_refresh_tried_again_says_retrying_not_needs_sign_in', () => {
  const c = { id: 'x', name: 'x', testing: false, test: { at: 1, ok: true, count: 2, retrying: true } } as unknown as ConnectorView;
  expect(statusOf(c, false).kind).toBe('retrying');
  expect(statusOf({ ...c, test: { at: 1, ok: true, count: 2 } }, false).kind).toBe('ok');
});

test('the_row_says_which_harness_keeps_its_own_server_and_which_cant_reach_it', () => {
  const host = mount(() => <RowNotes connector={{ name: 'linear', own: ['codex'], cant_reach: [] }} names={new Map()} />);
  expect([...host.querySelectorAll('[data-part="connectors.row.note"]')].map((n) => n.textContent)).toEqual(['Codex uses its own linear']);
  dispose?.();
  const none = mount(() => <RowNotes connector={{ name: 'linear', own: [], cant_reach: null }} names={new Map()} />);
  expect(none.querySelectorAll('[data-part="connectors.row.note"]').length).toBe(0);
  const sse = mount(() => <RowNotes connector={{ name: 'old', own: [], cant_reach: ['codex'] }} names={new Map()} />);
  expect(sse.textContent).toContain("Codex can't reach an SSE server");
});

test('show_hidden_lists_what_not_now_hid_so_it_can_still_be_added', async () => {
  const found: FoundServer[] = [
    { name: 'docs', from: ['Codex'], transport: 'local', command: 'docs-mcp', args: [] },
    { name: 'linear', from: ['Claude Code'], transport: 'remote', url: 'https://mcp.linear.app/mcp', hidden: true },
  ];
  let added: string[] = [];
  mount(() => <Review open found={found} onImport={(n) => (added = n)} onClose={() => {}} />);
  await Promise.resolve();
  const rows = (): (string | null)[] => [...document.querySelectorAll('[data-part="connectors.review.row"] [aria-label]')].map((x) => x.getAttribute('aria-label'));
  expect(rows()).toEqual(['docs']);
  const show = document.querySelector<HTMLElement>('[data-part="connectors.review.showHidden"]');
  expect(show?.textContent).toBe('Show 1 hidden');
  show?.click();
  await Promise.resolve();
  expect(rows()).toEqual(['docs', 'linear']);
  // shown, it isn't ticked for you: Add takes what you tick
  document.querySelector<HTMLElement>('[data-part="connectors.review.add"]')?.click();
  expect(added).toEqual(['docs']);
});

// UI QA r1: the notes were cut with "…" and had no title, so on a phone the reason (its own server, SSE) couldn't be
// read. Each note is the whole sentence, wrapping as it needs.
test('a_rows_notes_wrap_whole_instead_of_being_cut', () => {
  const name = 'a-connector-with-a-rather-long-name-that-would-not-fit-on-a-phone';
  const host = mount(() => <RowNotes connector={{ name, own: ['claude'], cant_reach: ['codex'] }} names={new Map()} />);
  const notes = [...host.querySelectorAll('[data-part="connectors.row.note"]')];
  expect(notes.map((n) => n.textContent)).toEqual([`Claude Code uses its own ${name}`, "Codex can't reach an SSE server, so it goes without"]);
  for (const n of notes) expect(n.hasAttribute('data-truncate')).toBe(false);
});
