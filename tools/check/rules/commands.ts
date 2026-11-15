// commands: a documented command that doesn't work never ships again (docs/v2-queue.md "Developer
// experience" 2: `npm run dev` pointed at a missing tools/dev.ts for weeks).
//   scripts  every script of the root package.json and of every workspace that runs a repo file
//            (`node tools/x.ts`, `tsx x.ts`, `./scripts/x`, `bash x.sh`), a crate (`cargo … -p <crate>`),
//            another script (`npm run x`, `npm test`) or a workspace (`--workspace <name>`) names one that exists
//   docs     the same for every command in AGENTS.md, README.md, ARCHITECTURE.md, docs/dev.md and
//            .claude/skills/*/SKILL.md: fenced sh/bash/console (or untagged) blocks and inline code spans;
//            `wisp <word>` must be a word of the CLI's own list (crates/wisp/src/args.rs: COMMANDS and the
//            usage text; main.rs's dispatch arms) or a plugin's command namespace (`wisp crew create` ↔ crew.create), else the CLI
//            launches it as a task. External tools (git, gh, curl, cargo's own subcommands) are skipped,
//            and so are placeholders (`<crate>`, `{x}`, `$X`, `*`, `…`).
// Paths resolve against what CI will check out (`git ls-files --cached --others --exclude-standard`, one
// call per run): a git-ignored path that exists only locally (qa/out/, target/) fails here too, with a
// message saying so. A path a documented command creates goes in CREATED with the command that makes it.
// Outside a git checkout (test fixtures without `git init`) the working tree is used.
// A docs line opts out with `<!-- cmd-check: skip <why> -->` on the line, or on the line just above a
// fence for the whole block; a skip without a why is an error.

import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync } from 'node:fs';
import { join, posix } from 'node:path';
import { finding } from '../core/report.ts';
import { crates } from '../core/rust.ts';
import { isRecord, readJson } from '../core/workspace.ts';
import type { Ctx, Finding, Rule } from '../core/types.ts';

const ID = 'commands';
const ARGS = 'crates/wisp/src/args.rs';
const MAIN = 'crates/wisp/src/main.rs';
const DOCS = ['AGENTS.md', 'README.md', 'ARCHITECTURE.md', 'docs/dev.md'];
/** fence languages whose lines are commands ('' = untagged: only lines that start with a known head) */
const SHELL = new Set(['sh', 'bash', 'shell', 'zsh', 'console', 'shell-session', '']);
/** a first word that is a path to run: `./x`, `../x` or one of these folders */
const RUNNABLE = /^(\.{1,2}\/|(\.agents|qa|tools|scripts|bin|spikes|ci)\/)/;
/** files a documented command runs that are installed, not committed: the installer must exist */
const INSTALLED: Readonly<Record<string, string>> = { '.agents/bin/monoflow': 'tools/setup-monoflow.ts', '.agents/bin/monoflow-bin': 'tools/setup-monoflow.ts' };
/** root scripts another coder is adding right now (coder-r230, docs/v2-queue.md DX 1): a doc may name them
 * before they land (their own bodies are checked like any script); drop once both are in package.json */
const PENDING_ROOT = new Set(['dev', 'dev:desktop']);
/** paths that are no files in git but that a documented command creates: path → the command that makes it */
const CREATED: Readonly<Record<string, string>> = {};
const SKIP = /<!--\s*cmd-check:\s*skip\b(.*?)-->/;

export interface Word {
  text: string;
  quoted: boolean;
}

/** Shell-ish split of one command line into segments (`&&`, `||`, `;`, `|`), each a list of words. An
 * unquoted `#` starts a comment. Good enough for documented commands, not a shell. */
export function segments(line: string): Word[][] {
  const out: Word[][] = [[]];
  let cur = '';
  let quoted = false;
  let has = false;
  let q: string | undefined;
  const push = (): void => {
    if (has) out[out.length - 1]?.push({ text: cur, quoted });
    cur = '';
    quoted = false;
    has = false;
  };
  for (let i = 0; i < line.length; i++) {
    const c = line[i] ?? '';
    if (q) {
      if (c === q) q = undefined;
      else cur += c;
      continue;
    }
    if (c === '"' || c === "'") {
      q = c;
      quoted = true;
      has = true;
    } else if (c === '#' && !has) break;
    else if (/\s/.test(c) || c === '(' || c === ')') push();
    else if (c === ';' || c === '|' || c === '&') {
      push();
      if (line[i + 1] === c) i++;
      out.push([]);
    } else {
      cur += c;
      has = true;
    }
  }
  push();
  return out.filter((s) => s.length > 0);
}

const placeholder = (s: string): boolean => /[<>{}$*…[\]]|\.\.\./.test(s) || s === '';
const flag = (s: string): boolean => s.startsWith('-');

/** What commands may name, read once per run. */
export interface Known {
  /** package dir ('' = root) → its script names */
  scripts: Map<string, Set<string>>;
  /** workspace name or dir → dir */
  workspaces: Map<string, string>;
  crates: Set<string>;
  /** the first words `wisp` takes */
  wisp: Set<string>;
  exists(file: string): boolean;
  /** on disk but not what CI sees (git-ignored); optional so hand-built Knowns stay valid */
  ignored?(file: string): boolean;
}

/** Every path CI will see (files and their folders), or undefined outside a git checkout. */
export function trackedPaths(root: string): Set<string> | undefined {
  let out: string;
  try {
    // root is the checkout's top when git sees no prefix: no comparing paths, which differ on Windows (git prints
    // C:/Users/runneradmin/…, os.tmpdir() gives C:\Users\RUNNER~1\… and realpathSync keeps the 8.3 name)
    const prefix = execFileSync('git', ['rev-parse', '--show-prefix'], { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
    if (prefix !== '') return undefined;
    out = execFileSync('git', ['ls-files', '-z', '--cached', '--others', '--exclude-standard'], { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, stdio: ['ignore', 'pipe', 'ignore'] });
  } catch {
    return undefined;
  }
  const set = new Set<string>();
  for (const f of out.split('\0')) {
    if (!f) continue;
    set.add(f);
    for (let i = f.lastIndexOf('/'); i > 0; i = f.lastIndexOf('/', i - 1)) {
      const d = f.slice(0, i);
      if (set.has(d)) break;
      set.add(d);
    }
  }
  return set;
}

function scriptsOf(manifest: unknown): Record<string, string> {
  const s = isRecord(manifest) ? manifest.scripts : undefined;
  return isRecord(s) ? Object.fromEntries(Object.entries(s).filter((e): e is [string, string] => typeof e[1] === 'string')) : {};
}

/** COMMANDS, the `wisp <word>` lines of the usage text, main.rs's dispatch arms (`"tasks" if … =>`,
 * `Some("grid") =>`) and every plugin's command namespace. */
export function wispWords(argsRs: string, mainRs: string, namespaces: Iterable<string>): Set<string> {
  const out = new Set<string>(namespaces);
  for (const m of mainRs.matchAll(/^\s*(?:Some\()?((?:"[a-z][a-z-]*"\s*\|?\s*)+)\)?\s*(?:if .*)?=>/gm)) for (const w of (m[1] ?? '').matchAll(/"([^"]+)"/g)) if (w[1]) out.add(w[1]);
  const list = /pub const COMMANDS:[^=]*=\s*\[([^\]]*)\]/.exec(argsRs)?.[1] ?? '';
  for (const m of list.matchAll(/"([^"]+)"/g)) if (m[1]) out.add(m[1]);
  const help = /pub const HELP: &str = "([\s\S]*?)";\n/.exec(argsRs)?.[1] ?? '';
  for (const m of help.matchAll(/^\s*wisp ([a-z][a-z-]*(?:\|[a-z][a-z-]*)*)/gm)) for (const w of (m[1] ?? '').split('|')) out.add(w);
  return out;
}

function readKnown(ctx: Ctx): Known {
  const scripts = new Map<string, Set<string>>([['', new Set(Object.keys(scriptsOf(readJson(join(ctx.root, 'package.json')))))]]);
  const workspaces = new Map<string, string>();
  for (const p of ctx.pkgs) {
    scripts.set(p.dir, new Set(Object.keys(scriptsOf(p.manifest))));
    workspaces.set(p.name, p.dir);
    workspaces.set(p.dir, p.dir);
    workspaces.set(`./${p.dir}`, p.dir);
  }
  const names = existsSync(join(ctx.root, 'Cargo.toml')) ? crates(ctx.root).map((c) => c.name) : [];
  const namespaces = new Set<string>();
  const plugins = existsSync(join(ctx.root, 'plugins')) ? readdirSync(join(ctx.root, 'plugins')) : [];
  for (const p of plugins) {
    const file = join(ctx.root, 'plugins', p, 'host', 'wisp-plugin.json');
    const m = existsSync(file) ? readJson(file) : undefined;
    const cmds = isRecord(m) && isRecord(m.contributes) && Array.isArray(m.contributes.commands) ? m.contributes.commands : [];
    for (const c of cmds) if (isRecord(c) && typeof c.id === 'string') namespaces.add(c.id.split('.')[0] ?? '');
  }
  const src = (f: string): string => (ctx.exists(f) ? ctx.read(f) : '');
  const tracked = trackedPaths(ctx.root);
  const strip = (f: string): string => f.replace(/\/+$/, '');
  const inGit = (f: string): boolean => (tracked ? strip(f) === '.' || strip(f) === '' || tracked.has(strip(f)) || CREATED[strip(f)] !== undefined : ctx.exists(f));
  const exists = (f: string): boolean => inGit(INSTALLED[f] ?? f);
  const ignored = (f: string): boolean => tracked !== undefined && ctx.exists(f);
  return { scripts, workspaces, crates: new Set(names), wisp: wispWords(src(ARGS), src(MAIN), namespaces), exists, ignored };
}

function checkPath(file: string, base: string, k: Known): string | undefined {
  if (placeholder(file) || file.startsWith('/') || file.startsWith('~') || file.includes(':')) return undefined;
  const rel = posix.normalize(posix.join(base, file));
  if (k.exists(rel)) return undefined;
  const from = base ? ` (from ${base})` : '';
  return k.ignored?.(rel) ? `${file}${from} is git-ignored: it exists here but won't in CI (name a tracked file, or add it to CREATED in ${'tools/check/rules/commands.ts'})` : `${file} doesn't exist${from}`;
}

function checkNpm(w: readonly string[], base: string, k: Known): string | undefined {
  let dir = base;
  let all = false;
  const rest: string[] = [];
  for (let i = 1; i < w.length; i++) {
    const a = w[i] ?? '';
    if (a === '--') break;
    const ws = a === '--workspace' || a === '-w' ? w[++i] : a.startsWith('--workspace=') ? a.slice(12) : undefined;
    if (ws !== undefined) {
      if (placeholder(ws)) return undefined;
      const d = k.workspaces.get(ws.replace(/\/$/, ''));
      if (d === undefined) return `npm workspace ${ws} doesn't exist`;
      dir = d;
    } else if (a === '--workspaces' || a === '-ws' || a === '--if-present') all = true;
    else if (!flag(a)) rest.push(a);
  }
  const [sub, name] = rest;
  const script = sub === 'run' || sub === 'run-script' ? name : sub === 'test' || sub === 't' || sub === 'start' ? (sub === 't' ? 'test' : sub) : undefined;
  if (!script || all || placeholder(script)) return undefined;
  if (dir === '' && PENDING_ROOT.has(script)) return undefined;
  return k.scripts.get(dir)?.has(script) ? undefined : `npm script ${script} doesn't exist in ${dir || 'the root'} package.json`;
}

const NODE_VALUE = new Set(['--import', '-r', '--require', '--loader', '--experimental-loader', '--env-file', '--conditions', '-C', '--title', '--input-type', '--stack-size', '--max-old-space-size', '--watch-path']);
/** `--test-name-pattern x`, `--test-reporter spec`, …: every --test-* option but --test-only takes a value */
const nodeValue = (a: string): boolean => NODE_VALUE.has(a) || (a.startsWith('--test-') && a !== '--test-only' && !a.includes('='));

function checkNode(w: readonly string[], base: string, k: Known): string | undefined {
  let test = false;
  for (let i = 1; i < w.length; i++) {
    const a = w[i] ?? '';
    if (a === '-e' || a === '--eval' || a === '-p' || a === '--print' || a === '-') return undefined;
    if (nodeValue(a)) i++;
    else if (a === '--test') test = true;
    else if (!flag(a)) {
      const bad = checkPath(a, base, k);
      if (bad || !test) return bad;
    }
  }
  return undefined;
}

function checkCargo(w: readonly string[], k: Known): string | undefined {
  for (let i = 2; i < w.length; i++) {
    const a = w[i] ?? '';
    if (a === '--') {
      const crate = pkgArg(w);
      return w[1] === 'run' && crate === 'wisp' ? checkWisp(w.slice(i), k) : undefined;
    }
    const name = a === '-p' || a === '--package' ? w[i + 1] : a.startsWith('--package=') ? a.slice(10) : undefined;
    if (name !== undefined && !placeholder(name) && !k.crates.has(name)) return `crate ${name} is no member of the Cargo workspace`;
  }
  return undefined;
}

function pkgArg(w: readonly string[]): string | undefined {
  const i = w.findIndex((a) => a === '-p' || a === '--package');
  return i >= 0 ? w[i + 1] : undefined;
}

/** `wisp <word> …`: the first word, when it's a bare word, must be one the CLI takes. */
function checkWisp(w: readonly string[], k: Known, quoted = false): string | undefined {
  const first = w[1];
  // `wisp v2`, `wisp 2.0`, `wisp:` in prose or sample output are no command
  if (!first || quoted || !/^[a-z][a-z-]*$/.test(first)) return undefined;
  return k.wisp.has(first) ? undefined : `wisp ${first} is no command of the CLI (${ARGS}, ${MAIN}): it would launch a task named "${first}"`;
}

/** One segment: its head decides what it names; anything else is an external tool. */
export function checkSegment(seg: readonly Word[], base: string, k: Known): string | undefined {
  let s = seg;
  while (s[0] && /^[A-Za-z_][A-Za-z0-9_]*=/.test(s[0].text)) s = s.slice(1);
  while (s[0] && ['sudo', 'exec', 'time', 'env', '$'].includes(s[0].text)) s = s.slice(1);
  const w = s.map((x) => x.text);
  const head = w[0] ?? '';
  if (head === 'npm') return checkNpm(w, base, k);
  if (head === 'node' || head === 'tsx') return checkNode(w, base, k);
  if (head === 'cargo') return checkCargo(w, k);
  if (head === 'wisp') return checkWisp(w, k, s[1]?.quoted ?? false);
  if ((head === 'bash' || head === 'sh' || head === 'zsh') && w[1] && (w[1].includes('/') || w[1].endsWith('.sh'))) return checkPath(w[1], base, k);
  if (RUNNABLE.test(head) && !s[0]?.quoted) return checkPath(head, base, k);
  return undefined;
}

/** Every segment of one line; a `cd` ends the checks (paths after it are relative to elsewhere): `cd`
 * says whether one was seen, so a block stops checking its later lines too. */
function checkLine(line: string, base: string, k: Known): { bad: string[]; cd: boolean } {
  const bad: string[] = [];
  for (const seg of segments(line)) {
    if (seg[0]?.text === 'cd') return { bad, cd: true };
    const b = checkSegment(seg, base, k);
    if (b) bad.push(b);
  }
  return { bad, cd: false };
}

function checkScripts(ctx: Ctx, k: Known): Finding[] {
  const out: Finding[] = [];
  const pkgs = [{ dir: '', file: 'package.json', manifest: readJson(join(ctx.root, 'package.json')) }, ...ctx.pkgs.map((p) => ({ dir: p.dir, file: `${p.dir}/package.json`, manifest: p.manifest as unknown }))];
  for (const p of pkgs) {
    const lines = ctx.exists(p.file) ? ctx.read(p.file).split('\n') : [];
    for (const [name, cmd] of Object.entries(scriptsOf(p.manifest))) {
      const line = lines.findIndex((l) => l.includes(`"${name}":`)) + 1 || 1;
      for (const bad of checkLine(cmd, p.dir, k).bad) out.push(finding(p.file, line, ID, `script ${name}: ${bad}`));
    }
  }
  return out;
}

export function docFiles(root: string): string[] {
  const skills = join(root, '.claude', 'skills');
  const list = existsSync(skills) ? readdirSync(skills).map((d) => `.claude/skills/${d}/SKILL.md`) : [];
  return [...DOCS, ...list.sort()].filter((f) => existsSync(join(root, f)));
}

/** Findings for one markdown file: shell fences line by line, inline code spans elsewhere. */
export function checkDoc(file: string, text: string, k: Known): Finding[] {
  const out: Finding[] = [];
  const lines = text.split('\n');
  let fence: { lang: string; skip: boolean } | undefined;
  // the syntax quoted in a code span (`<!-- cmd-check: skip <why> -->`) is no opt-out
  const skipOf = (l: string): RegExpExecArray | null => SKIP.exec(l.replace(/(`+)[^`]*?\1/g, ''));
  const flagSkip = (l: string, i: number): boolean => {
    const m = skipOf(l);
    if (m && !m[1]?.trim()) out.push(finding(file, i + 1, ID, 'cmd-check: skip needs a why (`<!-- cmd-check: skip <why> -->`)'));
    return Boolean(m);
  };
  lines.forEach((l, i) => {
    const f = /^\s*(```+|~~~+)\s*([\w-]*)/.exec(l);
    if (f) {
      // the line above was already read (and a skip there without a why reported) as a line of its own
      fence = fence ? undefined : { lang: (f[2] ?? '').toLowerCase(), skip: Boolean(skipOf(lines[i - 1] ?? '')) };
      return;
    }
    if (flagSkip(l, i)) return;
    const report = (bad: string): void => void out.push(finding(file, i + 1, ID, bad));
    if (fence) {
      if (fence.skip || !SHELL.has(fence.lang)) return;
      const res = checkLine(l, '', k);
      res.bad.forEach(report);
      if (res.cd) fence.skip = true;
      return;
    }
    for (const m of l.matchAll(/(`+)([^`]+?)\1/g)) checkLine(m[2] ?? '', '', k).bad.forEach(report);
  });
  return out;
}

export const commands: Rule = {
  id: ID,
  description: 'every package.json script and documented command names a file, script, workspace, crate or wisp command that exists',
  run(ctx) {
    const k = readKnown(ctx);
    const out = checkScripts(ctx, k);
    for (const f of docFiles(ctx.root)) out.push(...checkDoc(f, ctx.read(f), k));
    return out;
  },
};
