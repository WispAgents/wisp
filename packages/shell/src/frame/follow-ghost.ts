// Tab fills a follow-up (TRY-22, Gmail's Smart Compose for the reply box): the newest answer's suggested follow-ups
// (its `next` block) are what the box offers. Empty, it shows the first, faded, after the caret; typing narrows it to
// the first that starts with what's written (any case), and shows the rest; Tab takes it. DOM-free.

/** The follow-ups an answer suggests, in its order: each item's reply, else its label (the `next` block, v1 R.next). */
export function suggestedFollowUps(blocks: readonly unknown[] | undefined): string[] {
  const out: string[] = [];
  for (const b of blocks ?? []) {
    const block = b as { readonly type?: unknown; readonly items?: unknown };
    if (block?.type !== 'next' || !Array.isArray(block.items)) continue;
    for (const it of block.items) {
      const o = it as { readonly label?: unknown; readonly reply?: unknown } | string;
      const s = typeof o === 'string' ? o : typeof o?.reply === 'string' && o.reply ? o.reply : typeof o?.label === 'string' ? o.label : '';
      const one = s.trim();
      // one line each: the box completes a line, never pastes a paragraph behind the caret
      if (one && !one.includes('\n') && !out.includes(one)) out.push(one);
    }
  }
  return out;
}

/** What Tab would add after `typed`: the rest of the first suggestion that starts with it, or ''. Nothing on a second
 *  line, and nothing once it's all written. */
export function ghostOf(typed: string, suggestions: readonly string[]): string {
  return completionOf(typed, suggestions)?.slice(typed.length) ?? '';
}

/** The suggestion Tab writes for `typed`, as the answer wrote it (its case), or undefined. */
export function completionOf(typed: string, suggestions: readonly string[]): string | undefined {
  if (typed.includes('\n')) return undefined;
  const low = typed.toLowerCase();
  return suggestions.find((s) => s.length > typed.length && s.toLowerCase().startsWith(low));
}
