// Lightweight client-side parser for inline ```wisp blocks inside mid-turn chat messages (TimelineMessage).
// Fast path: returns undefined when no ```wisp opener is present so ordinary messages render plain <Markdown>.
// While a block's JSON is still streaming it is a trailing `{type:'pending', of?}` entry, so the message takes the
// Answer path from the opener on: no raw JSON shown as a code block, and no swap from <Markdown> to <Answer>
// (which would remount the prose above it) once the JSON closes.

const OPEN = '```wisp';
const FENCE = '```';

/** Finds the end index (exclusive) of a JSON object/array starting at `start`, respecting strings and escapes. */
function jsonEnd(src: string, start: number): number {
  let i = start;
  while (i < src.length && /\s/.test(src[i] ?? '')) i++;
  const ch = src[i];
  if (ch !== '{' && ch !== '[') return -1;
  let depth = 0;
  let inStr = false;
  for (; i < src.length; i++) {
    const c = src[i];
    if (inStr) {
      if (c === '\\') i++;
      else if (c === '"') inStr = false;
    } else if (c === '"') {
      inStr = true;
    } else if (c === '{' || c === '[') {
      depth++;
    } else if (c === '}' || c === ']') {
      depth--;
      if (depth === 0) return i + 1;
    }
  }
  return -1;
}

/** Whether the JSON from `start` runs to the end of `src` unclosed (still streaming), not malformed: it is empty so
 *  far or opens with `{`/`[`, and no closing fence line follows it (a closed fence of bad JSON is skipped, as before). */
function stillStreaming(src: string, start: number): boolean {
  const rest = src.slice(start);
  const lead = rest.trimStart();
  if (lead !== '' && lead[0] !== '{' && lead[0] !== '[') return false;
  return !/\n[ \t]*```[ \t]*(\n|$)/.test(rest);
}

/** The pending entry for a block still arriving: `of` is its first block's type, once that has arrived whole. */
function pending(partial: string): Record<string, unknown> {
  const of = /"type"\s*:\s*"([^"\\]+)"/.exec(partial)?.[1];
  return of ? { type: 'pending', of } : { type: 'pending' };
}

/** Splits a chat message containing ```wisp fences into ordered markdown + inline block objects, or undefined if none. */
export function parseInlineChatBlocks(text: string): readonly Record<string, unknown>[] | undefined {
  if (!text.includes(OPEN)) return undefined;
  const out: Record<string, unknown>[] = [];
  let cursor = 0;
  let search = 0;
  while (search < text.length) {
    const at = text.indexOf(OPEN, search);
    if (at < 0) break;
    const nl = text.indexOf('\n', at + OPEN.length);
    // the opener is the last thing so far: its block is on its way
    if (nl < 0) {
      if (text.slice(at + OPEN.length).trim() === '') return withPending(out, text.slice(cursor, at), '');
      break;
    }
    const body = nl + 1;
    const end = jsonEnd(text, body);
    if (end < 0) {
      if (stillStreaming(text, body)) return withPending(out, text.slice(cursor, at), text.slice(body));
      search = body;
      continue;
    }
    let parsed: unknown;
    try {
      parsed = JSON.parse(text.slice(body, end));
    } catch {
      search = body;
      continue;
    }
    const rawList = Array.isArray(parsed)
      ? parsed
      : parsed && typeof parsed === 'object' && Array.isArray((parsed as { blocks?: unknown }).blocks)
        ? (parsed as { blocks: unknown[] }).blocks
        : null;
    const valid = rawList?.filter((b): b is Record<string, unknown> => !!b && typeof b === 'object' && typeof (b as { type?: unknown }).type === 'string') ?? [];
    if (valid.length === 0) {
      search = body;
      continue;
    }
    const before = text.slice(cursor, at).trim();
    if (before) out.push({ type: 'markdown', text: before });
    out.push(...valid);
    const closeFence = text.indexOf(FENCE, end);
    if (closeFence >= 0) {
      const afterClose = text.indexOf('\n', closeFence + FENCE.length);
      cursor = afterClose >= 0 ? afterClose + 1 : text.length;
    } else {
      // the closer is still arriving: a lone ` or `` after the JSON is its start, not prose
      cursor = /^\s*`{0,2}\s*$/.test(text.slice(end)) ? text.length : end;
    }
    search = cursor;
  }
  if (out.length === 0) return undefined;
  const tail = text.slice(cursor).trim();
  if (tail) out.push({ type: 'markdown', text: tail });
  return out;
}

function withPending(out: Record<string, unknown>[], before: string, partial: string): readonly Record<string, unknown>[] {
  const prose = before.trim();
  if (prose) out.push({ type: 'markdown', text: prose });
  out.push(pending(partial));
  return out;
}
