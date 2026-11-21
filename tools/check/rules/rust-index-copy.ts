// rust-index-copy (git's racy-clean check vs a copied scratch index, round 188): git trusts an index entry whose
// file kept its recorded size and mtime unless the entry isn't older than the index file itself. `fs::copy` (std
// or tokio) gives the copy a fresh mtime on Linux, so a same-size edit made in the real index's own second reads as
// unchanged through the copy and is silently dropped (machines' carry lost a delegate's `a - b` → `a + b`). A
// scratch index is made with `wisp_proc::copy_index` / `copy_index_blocking` (the mtime kept, read from the same
// open file), or from an empty index (`read-tree HEAD`, re-hashed whole). Flagged: an `fs::copy(` in a function
// that names an index (an identifier with `index` in it, or a string `"index"` / `GIT_INDEX_FILE` /
// `--git-path`). A copy that is no git index says why in a `// index-copy: not git's <why>` comment on its line or
// the line above. crates/wisp-proc (the helper's home) and tests are exempt.

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { finding } from '../core/report.ts';
import { codeLines, crates, isTestFile, rustFiles, testStart } from '../core/rust.ts';
import type { Finding, Rule } from '../core/types.ts';

const HOME = 'crates/wisp-proc/';
/** `// index-copy: not git's <why>` (a reason given). */
const NOT_GITS = /\/\/\s*index-copy:\s*not git's\s+\S/;
/** `fs::copy(`, std's or tokio's, in a code line (comments and strings blanked). */
const COPY = /\bfs::copy\s*\(/;
/** The start of a function, in a code line. */
const FN = /\bfn\s+\w+/;
/** An identifier naming an index, in a code line. */
const IDENT = /[A-Za-z0-9_]*index[A-Za-z0-9_]*/i;
/** A string literal naming git's index, in a raw line. */
const STRING = /"(?:[^"\\]|\\.)*(?:\bindex\b|GIT_INDEX_FILE|--git-path)(?:[^"\\]|\\.)*"/;
/** How far past the call its arguments may run. */
const ARGS = 3;

/** Findings for one file's text: each `fs::copy(` of something an index is near, with no reason (exported for the rule's test). */
export function indexCopies(file: string, text: string): Finding[] {
  const raw = text.split('\n');
  const code = codeLines(text);
  const end = testStart(code);
  const out: Finding[] = [];
  for (let i = 0; i < end; i++) {
    if (!COPY.test(code[i] ?? '')) continue;
    let start = i;
    while (start > 0 && !FN.test(code[start] ?? '')) start--;
    const last = Math.min(end - 1, i + ARGS);
    const names = (l: number): boolean => IDENT.test(code[l] ?? '') || (STRING.test(raw[l] ?? '') && (code[l] ?? '').includes('""'));
    let near = false;
    for (let l = start; l <= last && !near; l++) near = names(l);
    if (!near) continue;
    const above = i > 0 && (code[i - 1] ?? '').trim() === '' ? (raw[i - 1] ?? '') : '';
    if (NOT_GITS.test(raw[i] ?? '') || NOT_GITS.test(above)) continue;
    out.push(
      finding(
        file,
        i + 1,
        'rust-index-copy',
        "copies what looks like a git index with fs::copy, which gives the copy a fresh mtime: git's racy-clean check then calls a same-size edit made in the index's own second unchanged, and it is dropped. Use wisp_proc::copy_index / copy_index_blocking (the mtime kept), or start from an empty index; a copy that is no git index says so in a `// index-copy: not git's <why>` comment",
        'error',
      ),
    );
  }
  return out;
}

export const rustIndexCopy: Rule = {
  id: 'rust-index-copy',
  description: 'a scratch git index is copied with wisp_proc::copy_index (the mtime kept), never fs::copy (racy-clean edits lost)',
  run(ctx) {
    const out: Finding[] = [];
    for (const c of crates(ctx.root).filter((x) => !`${x.dir}/`.startsWith(HOME))) {
      for (const f of rustFiles(ctx.root, `${c.dir}/src`).filter((x) => !isTestFile(x))) {
        out.push(...indexCopies(f, readFileSync(join(ctx.root, f), 'utf8')));
      }
    }
    return out;
  },
};
