// An agent's question as the ask card shows it: the call (headline), then what explains it (context).

/** The first paragraph is the call; anything after a blank line (or a list starting on the next line) is context. */
export function splitQuestion(q: string): { readonly headline: string; readonly context: string } {
  const text = q.trim();
  const m = /\n\s*\n|\n(?=\s*(?:[-*+]|\d+\.)\s)/.exec(text);
  if (m) return { headline: text.slice(0, m.index).trim(), context: text.slice(m.index).trim() };
  // one long paragraph: its first question is the call, what follows explains it
  const first = text.length > 160 ? /\?\s+(?=\S)/.exec(text) : null;
  if (first && first.index >= 20) return { headline: text.slice(0, first.index + 1), context: text.slice(first.index + first[0].length) };
  return { headline: text, context: '' };
}
