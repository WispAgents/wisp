// A run in words (QA r15 #3: "Runs on claude · opus · high" printed the raw ids): the harness's name, the model as
// the picker names it, the thinking level in words. Plain functions, no strings catalogue (they're on the first page);
// `useRunWords` (use-run-words.ts) gives a plugin the names and the default model from the shell.

/** "claude-haiku-5-5" → "Haiku 5.5", "claude-opus-5-5[1m]" → "Opus 5.5 (1M)", an alias capitalised (v1 modelWords). */
export function modelWords(id: string): string {
  const m = /^claude-(?:(\d+)-(?:(\d)-)?)?(opus|sonnet|haiku|fable)(?:-(\d{1,2})(?:-(\d{1,2}))?)?(?:-\d{8})?(\[1m\])?$/i.exec(id);
  const cap = (w: string): string => w.charAt(0).toUpperCase() + w.slice(1).toLowerCase();
  if (m?.[3]) {
    const v = m[4] ? `${m[4]}${m[5] ? `.${m[5]}` : ''}` : m[1] ? `${m[1]}${m[2] ? `.${m[2]}` : ''}` : '';
    return `${cap(m[3])}${v ? ` ${v}` : ''}${m[6] ? ' (1M)' : ''}`;
  }
  return /^(opus|sonnet|haiku|fable)$/i.test(id) ? cap(id) : id;
}

/** A thinking level in words: "High", "X-High"; '' for the harness's own. */
export const effortWord = (e: string): string => (e === 'xhigh' ? 'X-High' : e ? e.charAt(0).toUpperCase() + e.slice(1) : '');

/** "Claude Code · Opus · High": the harness's name, the model's words (none: its default, unnamed) and the thinking. */
export const runLabel = (r: { readonly name: string; readonly model?: string | undefined; readonly effort?: string | undefined }): string =>
  [r.name, r.model ?? '', effortWord(r.effort ?? '')].filter(Boolean).join(' · ');
