// rust-swallowed-write (store writes whose errors vanish): `let _ = store.write(|w| …).await;` or
// `….upsert(…).ok();` drops the store's refusal, so a save that failed looks like one that landed (the account
// comes back signed in after a restart, the pane's row never updates) and nothing says why. Round 203 found nine.
// A write either propagates its error (`?`, `map_err`) to a caller who can tell the user, or logs it with
// `tracing::warn!` naming what was being saved. The rare write whose result truly doesn't matter (an empty write
// used as a barrier, best-effort telemetry) says why in a `// write: best-effort <why>` comment in the comment lines
// right above it or on any line of the statement.
//
// A store write here is a `.write(` whose argument is a closure (`.write(|w| …)`, `.write(move |w| …)`: a child's
// stdin `.write(bytes)` is not one), an `.upsert(`, or a typed command whose name says it writes
// (`run::<…::SetExt>`, `Put…`, `Save…`, `Write…`, `Upsert…`, `Delete…`, `Remove…`, `Update…`, `Record…`,
// `Forget…`). It is swallowed when the statement is `let _ = …;` or an expression statement ending in `.ok();`
// (`let x = ….ok()`, `x = ….ok()` and `return ….ok()` keep the value). A plugin's own wrapper that returns the
// store's Result (`let _ = self.save(…)`) is not seen: name such wrappers' callers in review.
// Tests (`#[cfg(test)]` modules, tests/ files) are exempt.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { finding } from '../core/report.ts';
import { codeLines, crates, isTestFile, rustFiles, testStart } from '../core/rust.ts';
import type { Finding, Rule } from '../core/types.ts';

/** `// write: best-effort <why>` (a reason given). */
const BEST = /\/\/\s*write:\s*best-effort\s+\S/;
const WRITE = /\.write\(\s*(?:move\s*)?\||\.upsert\(|\brun::<\s*(?:\w+::)*(?:Set|Put|Save|Write|Upsert|Delete|Remove|Update|Record|Forget)\w*\s*>/;
/** A string literal: a reason written inside one is not a comment. */
const STR = /"(?:\\.|[^"\\])*"/g;
const LET_ = /\blet\s+_\s*=/g;
const OK_END = /\.ok\(\)\s*;/g;
/** A statement that keeps what `.ok()` gives: a binding, an assignment, a return. */
const KEEPS = /^(?:let\b|return\b|[\w.]+\s*=[^=])/;

const OPEN = '([{';
const CLOSE = ')]}';

/** Where the statement starting at `at` in `src` ends: the offset of its `;` at depth 0 (or the end of the text). */
function endOf(src: string, at: number): number {
  let depth = 0;
  for (let i = at; i < src.length; i++) {
    const c = src[i] ?? '';
    if (OPEN.includes(c)) depth++;
    else if (CLOSE.includes(c)) {
      if (depth === 0) return i;
      depth--;
    } else if (c === ';' && depth === 0) return i;
  }
  return src.length;
}

/** Where the statement ending at `at` starts: just after the `;`, `{` or `}` before it at depth 0. */
function startOf(src: string, at: number): number {
  let depth = 0;
  for (let i = at - 1; i >= 0; i--) {
    const c = src[i] ?? '';
    if (CLOSE.includes(c)) {
      if (c === '}' && depth === 0) return i + 1;
      depth++;
    } else if (OPEN.includes(c)) {
      if (depth === 0) return i + 1;
      depth--;
    } else if (c === ';' && depth === 0) return i + 1;
  }
  return 0;
}

/** The comment-only lines right above line `i` (raw text), nearest first. */
function commentsAbove(raw: readonly string[], code: readonly string[], i: number): string[] {
  const out: string[] = [];
  for (let j = i - 1; j >= 0 && (code[j] ?? '').trim() === '' && (raw[j] ?? '').trim().startsWith('//'); j--) out.push(raw[j] ?? '');
  return out;
}

/** Findings for one file's text: each swallowed store write with no reason (exported for the rule's test). */
export function swallowedWrites(file: string, text: string): Finding[] {
  const raw = text.split('\n');
  const code = codeLines(text);
  const end = testStart(code);
  const src = code.slice(0, end).join('\n');
  const lineAt = (offset: number): number => src.slice(0, offset).split('\n').length - 1;
  const spans: [number, number][] = [];
  for (const m of src.matchAll(LET_)) {
    const at = m.index ?? 0;
    spans.push([at, endOf(src, at + m[0].length)]);
  }
  for (const m of src.matchAll(OK_END)) {
    let at = startOf(src, m.index ?? 0);
    while (/\s/.test(src[at] ?? '')) at++;
    // `let _ = ….ok();` is found above
    if (KEEPS.test(src.slice(at))) continue;
    spans.push([at, (m.index ?? 0) + m[0].length - 1]);
  }
  const out: Finding[] = [];
  const seen = new Set<number>();
  for (const [from, to] of spans) {
    if (!WRITE.test(src.slice(from, to))) continue;
    const first = lineAt(from);
    if (seen.has(first)) continue;
    seen.add(first);
    const lines = [...commentsAbove(raw, code, first), ...raw.slice(first, lineAt(to) + 1)];
    if (lines.some((l) => BEST.test(l.replace(STR, '""')))) continue;
    out.push(
      finding(
        file,
        first + 1,
        'rust-swallowed-write',
        'a store write whose error is dropped (`let _ = …` or `….ok();`): propagate it (`?`, `map_err`) to a caller who can tell the user, or log it with tracing::warn! naming what was being saved, or say why it may fail unseen in a `// write: best-effort <why>` comment',
        'error',
      ),
    );
  }
  return out;
}

export const rustSwallowedWrite: Rule = {
  id: 'rust-swallowed-write',
  description: 'a store write never drops its error: propagate it, log it, or say why it is best-effort',
  run(ctx) {
    const out: Finding[] = [];
    for (const c of crates(ctx.root)) {
      for (const f of rustFiles(ctx.root, `${c.dir}/src`).filter((x) => !isTestFile(x))) {
        out.push(...swallowedWrites(f, readFileSync(join(ctx.root, f), 'utf8')));
      }
    }
    return out;
  },
};
