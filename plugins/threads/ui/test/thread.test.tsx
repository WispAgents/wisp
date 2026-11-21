// The Thread (variant A): the health line and Fresh thread, the sections with who wrote each and when, an inline
// edit (Enter saves, Shift+Enter a new line, Esc puts it back), the empty sections' Add row, a version brought back
// from History, and a lead's Notes read-only.
import { afterEach, expect, test, vi } from 'vitest';
import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import { t } from '../src/i18n.ts';
import '../src/i18n/lazy.ts';
import { sectionsOf, type SectionView } from '../src/model.ts';
import { ThreadView, type ThreadViewProps } from '../src/ThreadView.tsx';
import type { Health, NoteRow, Revision } from '../src/types.ts';

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});

const NOW = 10_000_000;
const row = (section: string, text: string, more: Partial<NoteRow> = {}): NoteRow => ({ id: `s1|${section}`, task_id: 's1', section, text, rev: 1, by: 'agent', at: NOW - 120_000, ...more });
const worn: Health = { task: 's1', level: 'worn', reasons: [], turns: 41, compactions: 3, share: 0.85, repeat: { sig: 'x', n: 4 }, reverted: 0 };

function mount(over: Partial<ThreadViewProps> = {}, sections: SectionView[] = sectionsOf([row('goal', 'Import CSV', { by: 'you', at: NOW - 3_600_000 }), row('constraints', 'No new dependencies\nKeep the API')], 's1')) {
  const onSave = vi.fn();
  const onFresh = vi.fn();
  const [s, setS] = createSignal(sections);
  const host = document.createElement('div');
  document.body.append(host);
  dispose = render(
    () => <ThreadView sections={s()} revisions={() => []} health={worn} readOnly={false} unseen={false} canFresh now={NOW} onSave={onSave} onFresh={onFresh} t={t} {...over} />,
    host,
  );
  const q = (part: string): HTMLElement[] => [...host.querySelectorAll<HTMLElement>(`[data-part="${part}"]`)];
  return { host, q, onSave, onFresh, setS };
}
const key = (el: Element, k: string, more: KeyboardEventInit = {}): boolean => el.dispatchEvent(new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true, ...more }));

test('the health line is one line: the level, what made it so, and Fresh thread when worn', () => {
  const { q, onFresh } = mount();
  expect(q('threads.health.level')[0]?.textContent).toBe('Worn');
  expect(q('threads.health.facts')[0]?.textContent).toBe('context 85% full · 3 compactions · same error 4× · 41 turns');
  q('threads.fresh')[0]?.click();
  expect(onFresh).toHaveBeenCalledTimes(1);
});

test('Fine shows no health line; without threads.fresh on the peer there is no Fresh thread', () => {
  const ok = mount({ health: { ...worn, level: 'ok' } });
  expect(ok.q('threads.health')).toEqual([]);
  dispose?.();
  const none = mount({ canFresh: false });
  expect(none.q('threads.health.level')[0]?.textContent).toBe('Worn');
  expect(none.q('threads.fresh')).toEqual([]);
});

test('each written section shows its text, who wrote it and when; the empty ones fold into one Add row', () => {
  const { q } = mount();
  expect(q('threads.section.label').map((n) => n.textContent)).toEqual(['Goal', 'Constraints']);
  expect(q('threads.section.by').map((n) => n.textContent)).toEqual(['you', 'agent']);
  expect(q('threads.section.time').map((n) => n.textContent)).toEqual(['1h', '2m']); // "agent · 2m" (the mock): the inline time
  expect(q('threads.section.text')[1]?.querySelectorAll('li').length).toBe(2);
  expect(q('threads.add.item').map((n) => n.textContent)).toEqual(['Decisions', 'Tried and failed', 'Where it stands', 'Open questions', 'Key files']);
});

test('the pen opens the section in place; Enter saves the text, Shift+Enter is a new line', () => {
  const { q, onSave } = mount();
  q('threads.section.edit')[0]?.click();
  const box = q('threads.section.input')[0] as HTMLTextAreaElement;
  expect(box.value).toBe('Import CSV');
  box.value = 'Import CSV files';
  box.dispatchEvent(new InputEvent('input', { bubbles: true }));
  expect(key(box, 'Enter', { shiftKey: true })).toBe(true); // not taken: the textarea adds the line
  key(box, 'Enter');
  expect(onSave).toHaveBeenCalledWith('goal', 'Import CSV files');
  expect(q('threads.section.input')).toEqual([]);
});

test('Esc puts the section back without saving', () => {
  const { q, onSave } = mount();
  q('threads.section.edit')[1]?.click();
  const box = q('threads.section.input')[0] as HTMLTextAreaElement;
  box.value = 'changed';
  box.dispatchEvent(new InputEvent('input', { bubbles: true }));
  key(box, 'Escape');
  expect(onSave).not.toHaveBeenCalled();
  expect(q('threads.section.input')).toEqual([]);
  expect(q('threads.section.text')[1]?.textContent).toContain('No new dependencies');
});

test('Add opens an empty section as a box in its place in the order; Save writes it', () => {
  const { q, onSave } = mount();
  q('threads.add.item')[2]?.click(); // Where it stands
  expect(q('threads.section.label').map((n) => n.textContent)).toEqual(['Goal', 'Constraints', 'Where it stands']);
  expect(q('threads.add.item').map((n) => n.textContent)).not.toContain('Where it stands');
  const box = q('threads.section.input')[0] as HTMLTextAreaElement;
  expect(box.placeholder).toBe('What\'s done, what\'s next');
  box.value = 'Parser done';
  box.dispatchEvent(new InputEvent('input', { bubbles: true }));
  q('threads.section.save')[0]?.click();
  expect(onSave).toHaveBeenCalledWith('state', 'Parser done');
});

test('a rewrite of one section updates it in place: the other sections keep their nodes', () => {
  const { q, setS } = mount();
  const before = q('threads.section');
  setS(sectionsOf([row('goal', 'Import CSV', { by: 'you', at: NOW - 3_600_000 }), row('constraints', 'No new dependencies', { rev: 2, by: 'you' })], 's1'));
  const after = q('threads.section');
  expect(after[0]).toBe(before[0]);
  expect(after[1]).toBe(before[1]);
  expect(q('threads.section.by')[1]?.textContent).toBe('you');
});

test('a section written after the clock last ticked reads as now, not as a time ahead of it', () => {
  const { q } = mount({}, sectionsOf([row('goal', 'G', { at: NOW + 20_000, by: 'you' })], 's1'));
  expect(q('threads.section.time')[0]?.textContent).toBe('now');
});

test('no Notes yet: the empty state says what they are for, and every section can be added', () => {
  const { q } = mount({}, sectionsOf([], 's1'));
  expect(q('threads.empty')[0]?.textContent).toContain('No Notes yet');
  expect(q('threads.add.item').length).toBe(7);
});

test('a lead’s Notes are read-only: no pen, no Add row, and it says so', () => {
  const { q } = mount({ readOnly: true });
  expect(q('threads.lead')[0]?.textContent).toContain('read-only');
  expect(q('threads.section.edit')).toEqual([]);
  expect(q('threads.add')).toEqual([]);
});

test('your change the agent hasn’t read yet says so', () => {
  expect(mount({ unseen: true }).q('threads.unseen')[0]?.textContent).toBe('The agent reads your changes with its next follow-up.');
});

test('History lists a section’s earlier versions; Use this version rewrites it with that text', async () => {
  const revs: Revision[] = [{ section: 'goal', rev: 1, text: 'Import files', by: 'agent', at: NOW - 7_200_000 }];
  const { q, onSave } = mount({ revisions: (s) => (s === 'goal' ? revs : []) });
  const trigger = q('threads.history.trigger')[0];
  expect(trigger).toBeTruthy();
  expect(q('threads.history.trigger').length).toBe(1); // only the section that has versions
  trigger?.click();
  await new Promise((r) => setTimeout(r, 0));
  const use = document.querySelector<HTMLElement>('[data-part="threads.history.use"]');
  expect(document.querySelector('[data-part="threads.history.text"]')?.textContent).toBe('Import files');
  use?.click();
  expect(onSave).toHaveBeenCalledWith('goal', 'Import files');
});

// QA r6 phone #10 (UX 2026-11-18 phone #14): on a phone the empty state's title is a few words; the why is body text
test('on_a_phone_no_notes_yet_is_a_short_title_and_its_why_as_body_text', () => {
  const { q } = mount({ phone: true }, sectionsOf([], 's1'));
  const title = q('threads.empty.title')[0]?.textContent ?? '';
  expect(title).toBe('No Notes yet');
  expect(title.split(/\s+/).length, 'a title, not a sentence').toBeLessThanOrEqual(4);
  expect(q('threads.empty.body')[0]?.textContent).toContain('outlast compaction');
  expect(q('threads.add.item').length, 'every section can still be added').toBe(7);
});
