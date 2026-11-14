// rust-canonicalize (Windows' verbatim paths, round 189): `std::fs::canonicalize` (tokio's, and `Path::canonicalize`)
// on Windows returns `\\?\C:\x` / `\\?\UNC\srv\share\x`. Git reads that as `//?/C:/x`, a host name ("hostname
// contains invalid characters": 13 tests on the first Windows CI run), shells refuse it, and it leaks into YAML,
// URLs and the UI. Every canonical path goes through `wisp_proc::canonical` / `canonical_blocking`, which strip the
// prefix. Flagged everywhere, tests included (that CI run failed in a test helper): a `canonicalize(` call in code
// (comments and strings blanked) outside crates/wisp-proc/src/canon.rs, unless a `// canonicalize: <why>`
// comment on its line or the line above says why (a crate that may not depend on wisp-proc, Unix-only code).

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { finding } from '../core/report.ts';
import { codeLines, crates, rustFiles } from '../core/rust.ts';
import type { Finding, Rule } from '../core/types.ts';

const HOME = 'crates/wisp-proc/src/canon.rs';
/** `// canonicalize: <why>` (a reason given). */
const WHY = /\/\/\s*canonicalize:\s*\S/;
/** `fs::canonicalize(` (std's or tokio's) or `.canonicalize(` (Path's), in a code line. */
const CALL = /(?:\bfs::canonicalize|\.canonicalize)\s*\(/;

/** Findings for one file's text: each canonicalize call with no reason (exported for the rule's test). */
export function canonicalizes(file: string, text: string): Finding[] {
  const raw = text.split('\n');
  const code = codeLines(text);
  const out: Finding[] = [];
  for (let i = 0; i < code.length; i++) {
    if (!CALL.test(code[i] ?? '')) continue;
    if (WHY.test(raw[i] ?? '') || (i > 0 && WHY.test(raw[i - 1] ?? ''))) continue;
    out.push(
      finding(
        file,
        i + 1,
        'rust-canonicalize',
        "canonicalizes with std/tokio, which on Windows returns a verbatim \\\\?\\C:\\… path that git reads as a host name and shells refuse. Use wisp_proc::canonical / canonical_blocking (the prefix stripped); code that can't says why in a `// canonicalize: <why>` comment",
        'error',
      ),
    );
  }
  return out;
}

export const rustCanonicalize: Rule = {
  id: 'rust-canonicalize',
  description: "a path is canonicalized only through wisp_proc::canonical (never Windows' \\\\?\\ verbatim form)",
  run(ctx) {
    const out: Finding[] = [];
    for (const c of crates(ctx.root)) {
      for (const f of rustFiles(ctx.root, c.dir).filter((x) => x !== HOME)) {
        out.push(...canonicalizes(f, readFileSync(join(ctx.root, f), 'utf8')));
      }
    }
    return out;
  },
};
