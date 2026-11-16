import { expect, test } from 'vitest';
import { applyPick, encodeMentions, itemsFor, menuAt, usePromptMenu, insertedTasks, putMentions, pastedTasks } from '../src/frame/prompt-menu.ts';
import type { PromptSourceDef } from '../src/contract.ts';
import { createRoot, createSignal } from 'solid-js';

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
