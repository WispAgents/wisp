// A ```wisp block streaming into a chat message (frame/chat-blocks.ts, TurnTranscript MessageBody): from the opener on
// the message is an Answer whose last entry is a pending skeleton, never the raw JSON as a code block, and the prose
// before it keeps its DOM node while the JSON arrives and once the block lands.
import { afterEach, expect, test } from 'vitest';
import { createSignal } from 'solid-js';
import { render } from 'solid-js/web';
import { parseInlineChatBlocks } from '../src/frame/chat-blocks.ts';
import { MessageBody } from '../src/frame/TurnTranscript.tsx';

const FULL = 'Here is the plan.\n\n```wisp\n{"blocks":[{"type":"chart","title":"Load","data":[1,2,3]}]}\n```\n\nDone.';
const OPENER_LINE = FULL.indexOf('```wisp') + '```wisp'.length;

let dispose: (() => void) | undefined;
afterEach(() => {
  dispose?.();
  dispose = undefined;
  document.body.innerHTML = '';
});

test('every prefix of a streaming wisp block: never undefined once the opener is in, no raw JSON, the prose first', () => {
  for (let n = OPENER_LINE; n <= FULL.length; n++) {
    const prefix = FULL.slice(0, n);
    const got = parseInlineChatBlocks(prefix);
    expect(got, prefix).toBeDefined();
    expect(got?.[0], prefix).toEqual({ type: 'markdown', text: 'Here is the plan.' });
    for (const e of got ?? []) {
      if (e['type'] !== 'markdown') continue;
      const text = String(e['text']);
      expect(text, prefix).not.toContain('{');
      expect(text, prefix).not.toContain('```');
      expect(text, prefix).not.toMatch(/^`+$/);
    }
  }
});

test('the pending entry names the block type once it has arrived whole', () => {
  expect(parseInlineChatBlocks('Hi\n\n```wisp\n{"blocks":[{"type":"cha')).toEqual([{ type: 'markdown', text: 'Hi' }, { type: 'pending' }]);
  expect(parseInlineChatBlocks('Hi\n\n```wisp\n{"blocks":[{"type":"chart","da')).toEqual([{ type: 'markdown', text: 'Hi' }, { type: 'pending', of: 'chart' }]);
  // a finished block before it stays as it was
  expect(parseInlineChatBlocks('```wisp\n[{"type":"tests","passed":1}]\n```\nthen\n```wisp\n[')).toEqual([
    { type: 'tests', passed: 1 },
    { type: 'markdown', text: 'then' },
    { type: 'pending' },
  ]);
});

test('a closed fence of malformed JSON still behaves as before', () => {
  expect(parseInlineChatBlocks('Hi\n\n```wisp\n{"blocks":[oops\n```\n')).toBeUndefined();
  expect(parseInlineChatBlocks('Hi\n\n```wisp\nnot json\n')).toBeUndefined();
});

test('the first paragraph keeps its DOM node while the JSON streams in and when the block lands', () => {
  const host = document.createElement('div');
  document.body.append(host);
  const [text, setText] = createSignal(FULL.slice(0, OPENER_LINE + 1));
  dispose = render(() => <MessageBody text={text()} here={(s) => s} chat answerCtx={{}} />, host);
  const first = host.querySelector('p');
  expect(first?.textContent).toBe('Here is the plan.');
  expect(host.querySelector('[data-kit="code-block"]')).toBeNull();
  expect(host.querySelector('[data-part="block.pending"]')).not.toBeNull();
  setText(FULL.slice(0, FULL.indexOf('"data"')));
  expect(host.querySelector('[data-part="block.pending"]')?.textContent).toContain('chart');
  expect(host.textContent).not.toContain('"blocks"');
  setText(FULL);
  expect(host.querySelector('[data-part="block.pending"]')).toBeNull();
  expect(host.querySelector('p')).toBe(first);
});
