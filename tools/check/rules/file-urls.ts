// Paths and file: URLs are not the same string on Windows. `new URL(rel, import.meta.url)` gives a URL whose
// `.pathname` is `/D:/a/wisp/...`: read as a path it becomes `D:\D:\a\...` (ENOENT); and a URL built by gluing
// a "file://" prefix onto a path (`D:\a\x`) is no URL at all, so a script's "am I the entry?" guard against
// process.argv[1] never matches and a page.goto() opens nothing. node:url converts both ways for every OS:
// fileURLToPath(new URL(rel, import.meta.url)) (or import.meta.dirname) for a path, pathToFileURL(p).href for
// a URL. Checked in every scanned file (packages, plugins, apps, tools) and in qa/ and mobile/scripts, the
// harnesses CI runs on Windows too.

import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { finding } from '../core/report.ts';
import type { Ctx, Finding, Rule } from '../core/types.ts';

const ID = 'file-urls';
const EXTRA = ['qa', 'mobile/scripts'];
const SCRIPT = /\.(ts|tsx|mts|cts|js|mjs|cjs)$/;
const CHECKS: readonly [RegExp, string][] = [
  [/import\.meta\.url\s*\)\s*\.pathname\b/, "a URL's .pathname used as a path is /D:/... on Windows: use fileURLToPath(new URL(…, import.meta.url))"],
  [/(['"])file:\/\/\/?\1\s*\+|`file:\/\/\/?\$\{/, 'a file: URL glued from a path is not a URL on Windows (D:\\…): use pathToFileURL(path).href'],
];

function extraFiles(root: string, dir: string, out: string[]): void {
  let entries;
  try {
    entries = readdirSync(join(root, dir), { withFileTypes: true });
  } catch {
    return;
  }
  for (const e of entries) {
    if (e.name.startsWith('.') || e.name === 'node_modules' || e.name === 'out' || e.name === 'dist') continue;
    const rel = `${dir}/${e.name}`;
    if (e.isDirectory()) extraFiles(root, rel, out);
    else if (e.isFile() && SCRIPT.test(e.name)) out.push(rel);
  }
}

export const fileUrls: Rule = {
  id: ID,
  description: 'paths come from fileURLToPath and file: URLs from pathToFileURL, never .pathname or a glued "file://" (Windows)',
  run(ctx: Ctx) {
    const files = new Set(ctx.files.filter((f) => SCRIPT.test(f) && !f.endsWith('.d.ts')));
    const more: string[] = [];
    for (const d of EXTRA) extraFiles(ctx.root, d, more);
    for (const f of more) files.add(f);
    const out: Finding[] = [];
    for (const file of [...files].sort()) {
      ctx.read(file).split('\n').forEach((l, i) => {
        for (const [re, why] of CHECKS) if (re.test(l)) out.push(finding(file, i + 1, ID, why));
      });
    }
    return out;
  },
};
