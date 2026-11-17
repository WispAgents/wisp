// The Thread tab's pure half (ADR-0079 §2, §6; v1 web/js/threads.js and features/thread.js): the seven sections in
// order with who wrote each, the text a rewrite keeps (as the server keeps it, so nothing jumps on the ack), the one
// health line, the revisions of a section, and which tasks have a thread and may start a fresh one.
import { describe, expect, test } from 'vitest';
import {
  SECTIONS,
  cleanText,
  freshable,
  healthFacts,
  isOne,
  itemsOf,
  noteId,
  revisionsOf,
  sectionsOf,
  shownText,
  splitSections,
  threadable,
  toneOf,
  yoursUnseen,
} from '../src/model.ts';
import type { Health, NoteRow } from '../src/types.ts';

const row = (section: string, text: string, more: Partial<NoteRow> = {}): NoteRow => ({ id: `s1|${section}`, task_id: 's1', section, text, rev: 1, by: 'agent', at: 1000, ...more });
const health = (more: Partial<Health> = {}): Health => ({ task: 's1', level: 'ok', reasons: [], turns: 4, compactions: 0, reverted: 0, ...more });

describe('sections', () => {
  test('the seven sections in v1 order, the ones a row is missing for empty', () => {
    expect(SECTIONS.map((s) => s.id)).toEqual(['goal', 'constraints', 'decisions', 'failed', 'state', 'open', 'files']);
    const s = sectionsOf([row('state', 'Parser done'), row('goal', 'Import CSV', { by: 'you', at: 5 }), row('other-task', 'x', { task_id: 's2' })], 's1');
    expect(s.map((x) => [x.id, x.text, x.by, x.rev])).toEqual([
      ['goal', 'Import CSV', 'you', 1],
      ['constraints', '', 'agent', 0],
      ['decisions', '', 'agent', 0],
      ['failed', '', 'agent', 0],
      ['state', 'Parser done', 'agent', 1],
      ['open', '', 'agent', 0],
      ['files', '', 'agent', 0],
    ]);
  });
  test('rows of another task are not this task’s; an unknown section is dropped', () => {
    expect(sectionsOf([row('goal', 'x', { task_id: 's2' }), row('mood', 'y')], 's1').every((x) => x.text === '')).toBe(true);
  });
  test('filled sections show, the empty ones fold into one Add row (variant A)', () => {
    const { filled, empty } = splitSections(sectionsOf([row('goal', 'G'), row('files', 'a.ts')], 's1'));
    expect(filled.map((x) => x.id)).toEqual(['goal', 'files']);
    expect(empty.map((x) => x.id)).toEqual(['constraints', 'decisions', 'failed', 'state', 'open']);
  });
  test('goal and state are one text; the rest are lists, one item a line', () => {
    expect(['goal', 'state'].every(isOne)).toBe(true);
    expect(['constraints', 'decisions', 'failed', 'open', 'files'].some(isOne)).toBe(false);
    expect(itemsOf('a\n\n b \nc')).toEqual(['a', 'b', 'c']);
    expect(shownText('constraints', 'No Redis\nKeep the API')).toBe('- No Redis\n- Keep the API');
    expect(shownText('goal', 'Import **CSV**')).toBe('Import **CSV**');
  });
  test('a section row’s id is `<task>|<section>`', () => {
    expect(noteId('s1', 'goal')).toBe('s1|goal');
  });
});

describe('what a rewrite keeps (as wisp-threads-core notes::clean)', () => {
  test('a list loses bullets, numbers and repeats (case and punctuation ignored)', () => {
    expect(cleanText('constraints', '- No Redis\n* no redis.\n• Keep the API\n2. Tests first\n12) Ship it\n\n')).toBe('No Redis\nKeep the API\nTests first\nShip it');
  });
  test('a list keeps its newest 20 items, each at most 400 characters', () => {
    const many = Array.from({ length: 25 }, (_, i) => `item ${i}`).join('\n');
    const kept = itemsOf(cleanText('decisions', many));
    expect(kept.length).toBe(20);
    expect(kept[0]).toBe('item 5');
    const long = cleanText('open', 'x'.repeat(500));
    expect([...long].length).toBe(400);
    expect(long.endsWith('…')).toBe(true);
  });
  test('a one-text section is trimmed, keeps its lines, and is cut at 1 200', () => {
    expect(cleanText('goal', '  Import CSV  \r\nwith headers   \n')).toBe('Import CSV\nwith headers');
    expect([...cleanText('state', 'y'.repeat(1300))].length).toBe(1200);
  });
  test('property: cleaning twice is cleaning once (the ack never changes what shows)', () => {
    const parts = ['- a', '* B', 'b.', '1. c', '  ', 'd ', 'D', '• e', '10) f', 'x'.repeat(450)];
    for (let seed = 1; seed < 60; seed++) {
      let s = seed;
      const lines = Array.from({ length: 1 + (seed % 9) }, () => {
        s = (s * 1103515245 + 12345) % 2147483648;
        return parts[s % parts.length] ?? '';
      });
      for (const id of ['goal', 'files']) {
        const once = cleanText(id, lines.join('\n'));
        expect(cleanText(id, once)).toBe(once);
      }
    }
  });
});

describe('health: one line', () => {
  test('Fine shows nothing; Getting long and Worn name what made them so, worst first', () => {
    expect(healthFacts(health())).toEqual([]);
    expect(healthFacts(health({ level: 'worn', share: 0.854, compactions: 3, repeat: { sig: 'x', n: 4 }, turns: 12 }))).toEqual([
      { key: 'threads.fact.context', vars: { pct: 85 } },
      { key: 'threads.fact.compactions', vars: { count: 3 } },
      { key: 'threads.fact.repeat', vars: { count: 4 } },
    ]);
  });
  test('turns, undone edits and old Notes count only past their thresholds (ADR-0079 §6)', () => {
    expect(healthFacts(health({ level: 'heavy', turns: 29, reverted: 1, notesAge: 7, share: 0.59, compactions: 1 }))).toEqual([]);
    expect(healthFacts(health({ level: 'heavy', turns: 30, reverted: 2, notesAge: 8 }))).toEqual([
      { key: 'threads.fact.turns', vars: { count: 30 } },
      { key: 'threads.fact.reverted', vars: { count: 2 } },
      { key: 'threads.fact.stale', vars: { count: 8 } },
    ]);
  });
  test('old Notes count only in a thread of 8 turns or more', () => {
    expect(healthFacts(health({ level: 'heavy', turns: 5, notesAge: 9 }))).toEqual([]);
  });
  test('the level’s tone: Fine good, Getting long warn, Worn bad', () => {
    expect([toneOf('ok'), toneOf('heavy'), toneOf('worn')]).toEqual(['good', 'warn', 'bad']);
  });
});

describe('revisions', () => {
  test('a section’s earlier versions, newest first, the current one left out', () => {
    const rows = [row('goal', 'one', { id: 's1|goal|1', rev: 1, at: 1 }), row('goal', 'three', { id: 's1|goal|3', rev: 3, at: 3 }), row('goal', 'two', { id: 's1|goal|2', rev: 2, at: 2, by: 'you' }), row('state', 's', { id: 's1|state|1' }), row('goal', 'other', { id: 's2|goal|1', task_id: 's2' })];
    expect(revisionsOf(rows, 's1', 'goal', 3).map((r) => [r.rev, r.text, r.by])).toEqual([
      [2, 'two', 'you'],
      [1, 'one', 'agent'],
    ]);
  });
});

describe('which tasks', () => {
  test('a task with an agent of its own has a thread: not the tour, a pending one or a spell', () => {
    expect(threadable({ agent: 'claude' })).toBe(true);
    expect(threadable({ agent: 'wisp' })).toBe(false);
    expect(threadable({ agent: 'claude', summary: { pending: true } })).toBe(false);
    expect(threadable({ agent: 'claude', summary: { kind: 'spell' } })).toBe(false);
    expect(threadable(undefined)).toBe(false);
    // as the task's menu hands it to an extension: flattened, camelCase
    expect(threadable({ agent: 'codex', pending: true })).toBe(false);
    expect(freshable({ agent: 'codex', state: 'review', parentId: 's0' })).toBe(false);
  });
  test('a fresh thread is for the task’s own conversation, once its turn has finished', () => {
    expect(freshable({ agent: 'claude', state: 'review' })).toBe(true);
    expect(freshable({ agent: 'claude', state: 'running' })).toBe(false);
    expect(freshable({ agent: 'claude', state: 'waiting' })).toBe(false);
    expect(freshable({ agent: 'claude', state: 'done', parent_id: 's0' })).toBe(false);
    expect(freshable({ agent: 'claude', state: 'done', summary: { sideOf: { id: 's0' } } })).toBe(false);
  });
  test('you changed them since the agent last read them: the Notes moved past what it saw, and a section is yours', () => {
    const s = sectionsOf([row('goal', 'G', { by: 'you' })], 's1');
    expect(yoursUnseen({ id: 's1', rev: 4, seen: 3 }, s)).toBe(true);
    expect(yoursUnseen({ id: 's1', rev: 4, seen: 4 }, s)).toBe(false);
    expect(yoursUnseen({ id: 's1', rev: 4, seen: 3 }, sectionsOf([row('goal', 'G')], 's1'))).toBe(false);
    expect(yoursUnseen(undefined, s)).toBe(false);
  });
});
