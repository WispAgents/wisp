// Lightweight client-side parser for inline ```wisp blocks inside mid-turn chat messages (TimelineMessage).
// Fast path: returns undefined when no ```wisp opener is present so ordinary messages render plain <Markdown>.

const OPEN = '```wisp';
/** A closing fence at the start of what follows the JSON: blank before it, and the rest of its line blank. */
const CLOSE = /^\s*```[ \t]*(\r?\n|$)/;

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
    if (nl < 0) break;
    const body = nl + 1;
    const end = jsonEnd(text, body);
    if (end < 0) {
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
    // The closing fence is the one right after the JSON (only whitespace between, nothing after it on its line). A
    // block left unclosed keeps the prose after it: searching on for any ``` would swallow it up to the next fence.
    const close = CLOSE.exec(text.slice(end));
    cursor = close ? end + close[0].length : end;
    search = cursor;
  }
  if (out.length === 0) return undefined;
  const tail = text.slice(cursor).trim();
  if (tail) out.push({ type: 'markdown', text: tail });
  return out;
}
