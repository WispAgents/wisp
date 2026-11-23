// The Rust side of the tree, read for the structure rules: the workspace's crates (from the root
// Cargo.toml's members), each crate's dependencies (from its Cargo.toml) and its .rs sources. The
// manifests are read as text, not through `cargo metadata`: the rules stay fast, need no toolchain, and
// run on the fixture trees the tests write. The reader handles the subset wisp's manifests use
// (`[section]` headers, `name = …`, `name.workspace = true`, inline tables with `package = "…"`).

import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join, posix } from 'node:path';

/** a plugin's crates: its contract, its sans-IO logic, its server side, and its terminal UI (ADR-0024) */
export type CrateRole = 'api' | 'core' | 'host' | 'tui';

export interface Crate {
  name: string;
  /** repo-relative, posix */
  dir: string;
  /** plugins/<plugin>/<role> */
  plugin?: string;
  role?: CrateRole;
  /** crates/* except the binary */
  base: boolean;
  /** normal and build dependencies, by crate name (renames resolved) */
  deps: string[];
  devDeps: string[];
  /** the line each dependency is declared on, for findings */
  depLine: Map<string, number>;
}

const SECTION = /^\s*\[([^\]]+)\]\s*$/;
const DEP = /^\s*([A-Za-z0-9_-]+)\s*(?:\.\s*workspace\s*)?=\s*(.*)$/;
const PACKAGE = /package\s*=\s*"([^"]+)"/;

function members(root: string): string[] {
  const text = readFileSync(join(root, 'Cargo.toml'), 'utf8');
  const list = (key: string): string[] => {
    const m = new RegExp(`^${key}\\s*=\\s*\\[([^\\]]*)\\]`, 'm').exec(text);
    return [...(m?.[1] ?? '').matchAll(/"([^"]+)"/g)].map((x) => x[1] ?? '');
  };
  const exclude = new Set(list('exclude'));
  const out: string[] = [];
  for (const pat of list('members')) {
    // one `*` per segment, as cargo's globs in this workspace use them
    let dirs = [''];
    for (const seg of pat.split('/')) {
      dirs = dirs.flatMap((d) => {
        if (!seg.includes('*')) return [posix.join(d, seg)];
        const at = join(root, d);
        if (!existsSync(at)) return [];
        const re = new RegExp(`^${seg.replace(/[.+?^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*')}$`);
        return readdirSync(at).filter((n) => re.test(n) && statSync(join(at, n)).isDirectory()).map((n) => posix.join(d, n));
      });
    }
    out.push(...dirs.filter((d) => !exclude.has(d) && existsSync(join(root, d, 'Cargo.toml'))));
  }
  return [...new Set(out)].sort();
}

function readCrate(root: string, dir: string): Crate {
  const lines = readFileSync(join(root, dir, 'Cargo.toml'), 'utf8').split('\n');
  let section = '';
  let name = posix.basename(dir);
  const deps: string[] = [];
  const devDeps: string[] = [];
  const depLine = new Map<string, number>();
  lines.forEach((line, i) => {
    const s = SECTION.exec(line);
    if (s?.[1]) {
      section = s[1].trim();
      return;
    }
    if (section === 'package') {
      const n = /^\s*name\s*=\s*"([^"]+)"/.exec(line);
      if (n?.[1]) name = n[1];
      return;
    }
    const kind = /(^|\.)(dependencies|build-dependencies)$/.test(section) ? deps : /(^|\.)dev-dependencies$/.test(section) ? devDeps : undefined;
    const d = kind && DEP.exec(line);
    if (!kind || !d?.[1]) return;
    const dep = PACKAGE.exec(d[2] ?? '')?.[1] ?? d[1];
    kind.push(dep);
    if (!depLine.has(dep)) depLine.set(dep, i + 1);
  });
  const p = /^plugins\/([^/]+)\/(api|core|host|tui)$/.exec(dir);
  const crate: Crate = { name, dir, base: /^crates\/[^/]+$/.test(dir) && dir !== 'crates/wisp', deps, devDeps, depLine };
  if (p?.[1] && p[2]) {
    crate.plugin = p[1];
    crate.role = p[2] as CrateRole;
  }
  return crate;
}

const cratesCache = new Map<string, Crate[]>();
const rustFilesCache = new Map<string, string[]>();
const rustTextCache = new Map<string, string>();
const codeLinesCache = new WeakMap<{ text: string }, string[]>();
const codeLinesByStr = new Map<string, string[]>();

/** Read a repo-relative file under `root`, cached per `(root, file)`. */
export function readRust(root: string, file: string): string {
  const key = `${root}\0${file}`;
  let t = rustTextCache.get(key);
  if (t === undefined) {
    t = readFileSync(join(root, file), 'utf8');
    rustTextCache.set(key, t);
  }
  return t;
}

export function crates(root: string): Crate[] {
  let c = cratesCache.get(root);
  if (!c) {
    c = existsSync(join(root, 'Cargo.toml')) ? members(root).map((d) => readCrate(root, d)) : [];
    cratesCache.set(root, c);
  }
  return c;
}

/** Every .rs file under `dir` (repo-relative), skipping build output. */
export function rustFiles(root: string, dir: string): string[] {
  const key = `${root}\0${dir}`;
  let cached = rustFilesCache.get(key);
  if (cached) return cached;
  const out: string[] = [];
  const walk = (d: string): void => {
    const at = join(root, d);
    if (!existsSync(at)) return;
    for (const e of readdirSync(at, { withFileTypes: true })) {
      const n = e.name;
      if (n === 'target' || n === 'node_modules' || n.startsWith('.')) continue;
      const rel = posix.join(d, n);
      if (e.isDirectory()) walk(rel);
      else if (n.endsWith('.rs')) out.push(rel);
    }
  };
  walk(dir);
  cached = out.sort();
  rustFilesCache.set(key, cached);
  return cached;
}

/** A test file: under tests/, or a `*_tests.rs` / `tests.rs` module beside the code. */
export const isTestFile = (file: string): boolean => /(^|\/)tests\//.test(file) || /(^|\/|_)tests\.rs$/.test(file);

/**
 * The code of each line with comments and string/char literals blanked (so `"unsafe"` in a message or
 * `// std::fs` in a comment is not code). Block comments and raw strings are handled; good enough for
 * keyword and brace scanning, not a parser.
 */
export function codeLines(text: string): string[] {
  void codeLinesCache;
  const hit = codeLinesByStr.get(text);
  if (hit) return hit;
  const out: string[] = [];
  let block = 0;
  let raw: string | undefined;
  for (const line of text.split('\n')) {
    let code = '';
    const len = line.length;
    for (let i = 0; i < len; i++) {
      if (raw !== undefined) {
        if (line.startsWith(raw, i)) {
          i += raw.length - 1;
          raw = undefined;
        }
        continue;
      }
      const c0 = line.charCodeAt(i);
      if (block) {
        if (c0 === 42 && line.charCodeAt(i + 1) === 47) {
          block--;
          i++;
        } else if (c0 === 47 && line.charCodeAt(i + 1) === 42) {
          block++;
          i++;
        }
        continue;
      }
      if (c0 === 47) {
        const c1 = line.charCodeAt(i + 1);
        if (c1 === 47) break;
        if (c1 === 42) {
          block++;
          i++;
          continue;
        }
      }
      if (c0 === 114 || (c0 === 98 && line.charCodeAt(i + 1) === 114)) {
        const prev = code.length > 0 ? code.charCodeAt(code.length - 1) : 0;
        const isIdent = (prev >= 48 && prev <= 57) || (prev >= 65 && prev <= 90) || (prev >= 97 && prev <= 122) || prev === 95;
        if (!isIdent) {
          let j = c0 === 98 ? i + 2 : i + 1;
          let hashes = 0;
          while (j < len && line.charCodeAt(j) === 35) {
            hashes++;
            j++;
          }
          if (j < len && line.charCodeAt(j) === 34) {
            raw = `"${'#'.repeat(hashes)}`;
            i = j;
            code += '""';
            continue;
          }
        }
      }
      if (c0 === 34) {
        let j = i + 1;
        while (j < len && line.charCodeAt(j) !== 34) j += line.charCodeAt(j) === 92 ? 2 : 1;
        i = j;
        code += '""';
        continue;
      }
      if (c0 === 39) {
        if (line.charCodeAt(i + 1) === 92) {
          const j = i + 2;
          if (j < len && line.charCodeAt(j + 1) === 39) {
            i = j + 1;
            code += "' '";
            continue;
          }
        } else if (i + 2 < len && line.charCodeAt(i + 1) !== 39 && line.charCodeAt(i + 2) === 39) {
          i += 2;
          code += "' '";
          continue;
        }
      }
      code += line[i];
    }
    out.push(code);
  }
  if (codeLinesByStr.size > 4000) codeLinesByStr.clear();
  codeLinesByStr.set(text, out);
  return out;
}

/** The index of the first `#[cfg(test)]` line: code from there on is the file's test module. */
export function testStart(lines: readonly string[]): number {
  const i = lines.findIndex((l) => /^\s*#\[cfg\(test\)\]/.test(l));
  return i < 0 ? lines.length : i;
}

/** The leading comment block (`//!`, `//`, blank lines) a file names its reasons in. */
export function header(text: string): string {
  const lines = text.split('\n');
  const end = lines.findIndex((l) => l.trim() !== '' && !l.trimStart().startsWith('//'));
  return lines.slice(0, end < 0 ? lines.length : end).join('\n');
}
