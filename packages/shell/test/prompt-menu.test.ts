import { expect, test } from 'vitest';
import { applyPick, decodeMentions, encodeMentions, itemsFor, menuAt, usePromptMenu, insertedTasks, putMentions, pastedTasks, titleOn, type Mentioned } from '../src/frame/prompt-menu.ts';
import type { PromptSourceDef } from '../src/contract.ts';
import { createRoot, createSignal } from 'solid-js';
import { signal } from '@wisp/client';

test('a_prompt_box_opens_its_menu_where_the_caret_is_and_sends_picked_mentions_in_v1s_form', () => {
  expect(menuAt('/go', 3)).toEqual({ trigger: '/', start: 0, query: 'go' });
  expect(menuAt('/goal ship it', 13)).toBeNull(); // past the command
  expect(menuAt('ask @Fi', 7)).toEqual({ trigger: '@', start: 4, query: 'Fi' });
  expect(menuAt('mail a@b', 8)).toBeNull(); // an address, not a mention
  expect(menuAt('@', 1)).toEqual({ trigger: '@', start: 0, query: '' });
  const r = applyPick('ask @Fi please', { trigger: '@', start: 4, query: 'Fi' }, 7, { insert: '@Fix the bug ' });
  expect(r).toEqual({ text: 'ask @Fix the bug  please', caret: 17 });
  expect(
    encodeMentions('see @Fix the bug and @Ada', [
      { scheme: 'wisp', id: 'a1', title: 'Fix the bug' },
      { scheme: 'crew', id: 'ada', title: 'Ada' },
    ]),
  ).toBe('see @[Fix the bug](wisp:a1) and @[Ada](crew:ada)');
  expect(encodeMentions('nothing named', [{ scheme: 'wisp', id: 'a1', title: 'Gone' }])).toBe('nothing named');
  const src = (id: string, trigger: '@' | '/', where?: ('reply' | 'home')[]): PromptSourceDef => ({
    id,
    trigger,
    ...(where && { where }),
    items: (q) => [{ value: `${id}:${q}`, label: id, insert: id }],
  });
  const ctx = { client: {} as never };
  expect(itemsFor([src('a', '@'), src('b', '/'), src('c', '/', ['home'])], { trigger: '/', start: 0, query: 'x' }, 'reply', ctx).map((i) => i.value)).toEqual(['b:x']);
});

test('a pasted task id becomes the task s mention; anything else is pasted as text_v1_paste_task_id', async () => {
  const titles: Record<string, string> = { '0192f3a4-aaaa-7bbb': 'Fix login', 'local:s0002': 'Ship it' };
  const title = (id: string): string | undefined => titles[id];
  expect(pastedTasks('0192f3a4-aaaa-7bbb', title)).toEqual([{ scheme: 'wisp', id: '0192f3a4-aaaa-7bbb', title: 'Fix login' }]);
  // several, a wisp: id, a link; each once
  expect(pastedTasks('wisp:0192f3a4-aaaa-7bbb http://x/#/s/local:s0002 0192f3a4-aaaa-7bbb', title)?.map((m) => m.title)).toEqual(['Fix login', 'Ship it']);
  // one that isn't a task here, or words: the box takes the text
  expect(pastedTasks('0192f3a4-aaaa-7bbb nope123', title)).toBeNull();
  expect(pastedTasks('fix the login bug', title)).toBeNull();
  // sent, it's the stored form the agent reads (wisp_session)
  expect(encodeMentions('see @Fix login first', pastedTasks('0192f3a4-aaaa-7bbb', title) ?? [])).toBe('see @[Fix login](wisp:0192f3a4-aaaa-7bbb) first');
});

test('a_task_id_a_phone_keyboards_clipboard_puts_in_becomes_its_mention_v1_800', async () => {
  const id = '0192f3a4aaaa7bbb8ccc9ddd0eee1fff';
  const title = (x: string): string | undefined => (x === id ? 'Fix login' : undefined);
  // Gboard's clipboard chip: the id lands whole in one insert, ending at the caret
  expect(insertedTasks(`see ${id}`, 4 + id.length, id, title)).toEqual({ from: 4, to: 4 + id.length, got: [{ scheme: 'wisp', id, title: 'Fix login' }] });
  // typing adds a letter at a time; a short insert (autocorrect's word) or words that aren't tasks stay text
  expect(insertedTasks(`see ${id}`, 4 + id.length, 'f', title)).toBeNull();
  expect(insertedTasks('see fix the login bug', 21, 'the login bug', title)).toBeNull();
  // the insert must be what sits before the caret
  expect(insertedTasks(`see ${id}`, 4, id, title)).toBeNull();
  // the span becomes @Title and a space; a space before it after a word (v1 put)
  expect(putMentions(`see${id} now`, 3, 3 + id.length, [{ scheme: 'wisp', id, title: 'Fix login' }])).toEqual({ text: 'see @Fix login  now', caret: 15 });
  expect(putMentions(id, 0, id.length, [{ scheme: 'wisp', id, title: 'Fix login' }])).toEqual({ text: '@Fix login ', caret: 11 });
});

test('the_prompt_menu_turns_a_clipboard_insert_with_no_paste_event_into_a_chip_and_sends_it_encoded_v1_800', async () => {
  const id = '0192f3a4aaaa7bbb8ccc9ddd0eee1fff';
  const shell = {
    promptSources: () => [],
    t: (k: string) => k,
    client: {
      plugin: () => ({ home: 'local' }),
      entities: { peek: (_kind: string, key: string) => (key === `local:${id}` ? { title: 'Fix login' } : undefined) },
    },
  };
  const el = document.createElement('textarea');
  document.body.append(el);
  await createRoot(async (dispose) => {
    const [text, setText] = createSignal('');
    const menu = usePromptMenu({ shell: shell as never, where: 'reply', text, setText, input: () => el, id: 'x' });
    // what the box does on input: its value, then the menu with the event
    const input = (value: string, e: InputEvent): string => {
      el.value = value;
      el.setSelectionRange(value.length, value.length);
      setText(value);
      return menu.typed(value, e);
    };
    // one letter typed: text as is
    expect(input('a', new InputEvent('input', { inputType: 'insertText', data: 'a' }))).toBe('a');
    // the keyboard's clipboard: the whole id in one insertText, no paste event
    expect(input(`a${id}`, new InputEvent('input', { inputType: 'insertText', data: id }))).toBe('a @Fix login ');
    expect(text()).toBe('a @Fix login ');
    expect(menu.encode(text())).toBe(`a @[Fix login](wisp:${id}) `);
    // composing (an IME word in progress) is left alone
    setText('');
    expect(input(id, new InputEvent('input', { inputType: 'insertText', data: id, isComposing: true }))).toBe(id);
    dispose();
  });
  el.remove();
});

// round 324: the menu's items follow what its sources read in the entity store (alien signals) and the sources the
// registry holds; a plain createMemo read both once, so an @ typed before the tasks landed, a task renamed while the
// menu showed, or a source a plugin's later part registers never reached the menu
test('the_prompt_menu_follows_rows_that_land_or_change_and_sources_registered_after_it_opened', () => {
  const tasks = signal<readonly { id: string; title: string }[]>([]);
  const at = (id: string): PromptSourceDef => ({ id, trigger: '@', items: (q) => tasks().filter((x) => x.title.toLowerCase().includes(q.toLowerCase())).map((x) => ({ value: `${id}:${x.id}`, label: x.title, insert: `@${x.title} ` })) });
  const sources = signal<readonly PromptSourceDef[]>([at('tasks')]);
  const shell = { promptSources: () => sources, t: (k: string) => k, client: { plugin: () => ({ home: 'local' }), entities: { peek: () => undefined } } };
  const el = document.createElement('textarea');
  document.body.append(el);
  createRoot((dispose) => {
    const [text, setText] = createSignal('');
    const menu = usePromptMenu({ shell: shell as never, where: 'reply', text, setText, input: () => el, id: 'x' });
    const type = (value: string): void => {
      el.value = value;
      el.setSelectionRange(value.length, value.length);
      setText(value);
      menu.typed(value);
    };
    const labels = (): string[] => menu.suggest()?.items.map((i) => i.label) ?? [];
    type('ask @fi');
    expect(labels(), 'no tasks yet').toEqual([]);
    // the list's rows land while the menu shows: the match appears without typing again
    tasks([{ id: 'a1', title: 'Fix login' }]);
    expect(labels(), 'the late task').toEqual(['Fix login']);
    // renamed elsewhere: the menu shows the new title
    tasks([{ id: 'a1', title: 'Fix the login page' }]);
    expect(labels(), 'the renamed task').toEqual(['Fix the login page']);
    // a source registered after (a plugin's later part): its items join
    sources([...sources(), { id: 'crew', trigger: '@', items: () => [{ value: 'crew:ada', label: 'Ada fixes', insert: '@Ada ' }] }]);
    expect(labels(), 'the late source').toEqual(['Fix the login page', 'Ada fixes']);
    // deleted: it goes
    tasks([]);
    expect(labels(), 'the deleted task').toEqual(['Ada fixes']);
    dispose();
  });
  el.remove();
});

// doc25a: a sent prompt is stored with its mentions encoded; put back in the box it shows `@Title` again and sends them
test('decoding_a_sent_prompt_gives_back_the_box_text_and_its_mentions_doc25a', () => {
  const ms: Mentioned[] = [
    { scheme: 'wisp', id: '0192f3a4-aaaa-7bbb', title: 'Fix: login (v2)!' },
    { scheme: 'crew', id: 'ada', title: 'Ada Lovelace' },
    { scheme: 'stuff', id: 's1', title: 'Release "checklist"' },
  ];
  const text = 'see @Fix: login (v2)! then @Ada Lovelace, and @Release "checklist".';
  const sent = encodeMentions(text, ms);
  expect(sent).toBe('see @[Fix: login (v2)!](wisp:0192f3a4-aaaa-7bbb) then @[Ada Lovelace](crew:ada), and @[Release "checklist"](stuff:s1).');
  expect(decodeMentions(sent)).toEqual({ text, mentions: ms });
  // plain text and half-written or foreign markup stay as they are
  for (const plain of ['nothing here', 'mail a@b.c', '@[open', '@[x](ftp:1)', '@[x](wisp:)', '@[](wisp:a)', '[x](wisp:a)']) expect(decodeMentions(plain)).toEqual({ text: plain, mentions: [] });
});

// doc25b: the longest title first, whole words only, every time, never inside what's already stored
test('encoding_matches_whole_words_longest_title_first_doc25b', () => {
  const a: Mentioned = { scheme: 'wisp', id: 'a', title: 'Auth' };
  const b: Mentioned = { scheme: 'wisp', id: 'b', title: 'Auth bug' };
  const want = '@[Auth bug](wisp:b) fix and @[Auth](wisp:a) ';
  expect(encodeMentions('@Auth bug fix and @Auth ', [a, b])).toBe(want);
  expect(encodeMentions('@Auth bug fix and @Auth ', [b, a])).toBe(want);
  expect(decodeMentions(want)).toEqual({ text: '@Auth bug fix and @Auth ', mentions: [b, a] });
  // another word, or not where @ opens the menu
  expect(encodeMentions('@Authentic and x@Auth and @Auth-flow', [a])).toBe('@Authentic and x@Auth and @Auth-flow');
  // every time it's there; at the end and before punctuation
  expect(encodeMentions('@Auth, then @Auth', [a])).toBe('@[Auth](wisp:a), then @[Auth](wisp:a)');
  // already stored (a title with an @ word in it): left alone
  expect(encodeMentions('@[Ask @Auth](wisp:z) ', [a])).toBe('@[Ask @Auth](wisp:z) ');
});

// doc25a: ↑'s history, a refused send and a reload give the box `@Title` and keep what it sends
test('a_recalled_or_restored_prompt_shows_titles_and_sends_the_same_mentions_doc25a', () => {
  const store = new Map<string, unknown>();
  const cell = (e: string, f: string): string => `${e}|${f}`;
  const drafts = {
    get: (e: string, f: string) => () => store.get(cell(e, f)),
    set: (e: string, f: string, v: unknown) => void (v === undefined ? store.delete(cell(e, f)) : store.set(cell(e, f), v)),
  };
  const shell = { promptSources: () => [], t: (k: string) => k, client: { plugin: () => ({ home: 'local' }), drafts, entities: { peek: () => undefined } } };
  const el = document.createElement('textarea');
  document.body.append(el);
  const sent = 'ask @[Fix login](wisp:a1) and @[Ada](crew:ada) ';
  createRoot((dispose) => {
    const [text, setText] = createSignal('');
    const menu = usePromptMenu({ shell: shell as never, where: 'reply', task: 'local:t1', draft: ['local:t1', 'reply'], text, setText, input: () => el, id: 'x' });
    // ↑ (or a refusal) puts the stored prompt back
    expect(menu.restore(sent)).toBe('ask @Fix login and @Ada ');
    expect(text()).not.toContain('](wisp:');
    expect(menu.encode(text())).toBe(sent);
    dispose();
  });
  // a reload, or coming back to the task: a new box on the same draft still sends them
  createRoot((dispose) => {
    const [text, setText] = createSignal('ask @Fix login and @Ada ');
    const menu = usePromptMenu({ shell: shell as never, where: 'reply', task: 'local:t1', draft: ['local:t1', 'reply'], text, setText, input: () => el, id: 'x' });
    expect(menu.encode(text())).toBe(sent);
    // sent: none are kept for the next prompt
    menu.reset();
    expect(menu.encode(text())).toBe(text());
    dispose();
  });
  el.remove();
});

// new1: a pasted id names a task on the computer the box sends to, never another's (its mention would be dropped there)
test('a_pasted_id_is_read_on_the_computer_the_box_sends_to_new1', () => {
  const rows: Record<string, string> = { 'a:t1': 'Fix login', 'b:t2': 'Ship it' };
  const peek = (k: string): string | undefined => rows[k];
  expect(titleOn('a', 't1', peek)).toBe('Fix login');
  expect(titleOn('a', 't2', peek), 'only on b').toBeUndefined();
  expect(titleOn('a', 'b:t2', peek), "b's key").toBeUndefined();
  expect(titleOn('b', 'b:t2', peek)).toBe('Ship it');
  expect(pastedTasks('t2', (id) => titleOn('a', id, peek))).toBeNull();
});

// new1: the menu asks its sources for the computer the box sends to: New task's Run on, else a reply's task's
test('the_prompt_menu_tells_its_sources_the_computer_it_sends_to_new1', () => {
  const seen: (string | undefined)[] = [];
  const src: PromptSourceDef = { id: 'p', trigger: '@', items: (_q, ctx) => (seen.push(ctx.peer), []) };
  const shell = { promptSources: () => () => [src], t: (k: string) => k, client: { plugin: () => ({ home: 'local' }), entities: { peek: () => undefined } } };
  const el = document.createElement('textarea');
  document.body.append(el);
  createRoot((dispose) => {
    const [text, setText] = createSignal('');
    const [runOn, setRunOn] = createSignal('box');
    const home = usePromptMenu({ shell: shell as never, where: 'home', peer: runOn, text, setText, input: () => el, id: 'h' });
    const reply = usePromptMenu({ shell: shell as never, where: 'reply', task: 'laptop:t1', text, setText, input: () => el, id: 'r' });
    el.value = '@';
    el.setSelectionRange(1, 1);
    home.typed('@');
    home.suggest();
    setRunOn('local');
    home.typed('@x');
    home.suggest();
    reply.typed('@');
    reply.suggest();
    // (each change reads the sources again: the computers asked for, in order)
    expect(seen.filter((p, i) => p !== seen[i - 1])).toEqual(['box', 'local', 'laptop']);
    dispose();
  });
  el.remove();
});
