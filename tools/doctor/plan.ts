// tools/doctor.ts's pure parts: the version compare, the disk threshold, the lockfile drift, the toolchain verdict,
// what blocks (exit 1) versus what is advice, and the report lines. No I/O here (tools/test/doctor.test.ts).

/** `fail` blocks (exit 1); `warn` is advice that keeps a part of the workflow from working; `info` is a note. */
export type Status = 'ok' | 'warn' | 'fail' | 'info';

export interface Check {
  id: string;
  title: string;
  status: Status;
  /** What was found, one line. */
  detail: string;
  /** What to run or do, when not ok. */
  fix?: string;
}

/** Free disk under the cargo target dir below this is a warning: one wisp workspace build is ~10 GB. */
export const DISK_WARN_BYTES = 20 * 1024 ** 3;
export const DEV_PORT = 4800;
/** An installed wisp: noticed, never touched. */
export const INSTALLED_PORT = 4477;

/** `22.18.0`, `v22.18`, `1.100.0-aarch64` → [22, 18, 0]; undefined when it doesn't start with a number. */
export function parseVersion(v: string): [number, number, number] | undefined {
  const m = /^v?(\d+)(?:\.(\d+))?(?:\.(\d+))?/.exec(v.trim());
  if (!m) return undefined;
  return [Number(m[1]), Number(m[2] ?? 0), Number(m[3] ?? 0)];
}

/** <0, 0, >0 like a sort comparator; an unparsable side sorts first. */
export function compareVersions(a: string, b: string): number {
  const x = parseVersion(a);
  const y = parseVersion(b);
  if (!x || !y) return (x ? 1 : 0) - (y ? 1 : 0);
  for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return (x[i] ?? 0) - (y[i] ?? 0);
  return 0;
}

type V = [number, number, number];

function cmp(a: V, b: V): number {
  return a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
}

/** A partial version (`22`, `22.18`) as the half-open x-range [lo, hi) it means; a full one has hi = lo + 1 patch. */
function xrange(want: string): { lo: V; hi: V; full: boolean } {
  const parts = want.split('.').map(Number);
  const [a = 0, b = 0, c = 0] = parts;
  const lo: V = [a, b, c];
  const hi: V = parts.length === 1 ? [a + 1, 0, 0] : parts.length === 2 ? [a, b + 1, 0] : [a, b, c + 1];
  return { lo, hi, full: parts.length === 3 };
}

/** One comparator (`>=22.18`, `^20.19`, `22`) against v; undefined when it isn't one this reads. */
function comparator(v: V, part: string): boolean | undefined {
  const m = /^(>=|<=|>|<|=|\^|~)?v?(\d+(?:\.\d+){0,2})(?:\.[xX*])*$/.exec(part);
  if (!m) return undefined;
  const op = m[1] ?? '=';
  const { lo, hi, full } = xrange(m[2] ?? '0');
  switch (op) {
    case '>=': return cmp(v, lo) >= 0;
    case '>': return full ? cmp(v, lo) > 0 : cmp(v, hi) >= 0;
    case '<': return cmp(v, lo) < 0;
    case '<=': return full ? cmp(v, lo) <= 0 : cmp(v, hi) < 0;
    case '^': {
      const top: V = lo[0] > 0 ? [lo[0] + 1, 0, 0] : lo[1] > 0 ? [0, lo[1] + 1, 0] : hi;
      return cmp(v, lo) >= 0 && cmp(v, top) < 0;
    }
    case '~': {
      const n = (m[2] ?? '').split('.').length;
      const top: V = n === 1 ? [lo[0] + 1, 0, 0] : [lo[0], lo[1] + 1, 0];
      return cmp(v, lo) >= 0 && cmp(v, top) < 0;
    }
    default: return cmp(v, lo) >= 0 && cmp(v, hi) < 0;
  }
}

/**
 * Whether `version` meets an npm engines range: `||` alternatives of space-separated comparators (`>=`, `>`, `<=`,
 * `<`, `=`/bare, `^`, `~`, partial versions as x-ranges, `A - B` hyphen ranges). A range (or an alternative) it can't
 * read (a tag, `*`) is accepted: the doctor never blocks on a range it can't read.
 */
export function satisfies(version: string, range: string): boolean {
  const v = parseVersion(version);
  const alts = range.split('||').map((s) => s.trim()).filter(Boolean);
  if (!v || !alts.length) return true;
  return alts.some((alt) => {
    const norm = alt
      .replace(/(>=|<=|>|<|=|\^|~)\s+/g, '$1')
      .replace(/^(\S+)\s+-\s+(\S+)$/, '>=$1 <=$2');
    const results = norm.split(/\s+/).map((p) => comparator(v, p));
    return results.includes(undefined) ? true : results.every(Boolean);
  });
}

export function nodeCheck(version: string, engines: string | undefined): Check {
  const base = { id: 'node', title: 'Node' };
  if (!engines) return { ...base, status: 'ok', detail: `${version} (package.json has no engines.node)` };
  return satisfies(version, engines)
    ? { ...base, status: 'ok', detail: `${version} meets engines ${engines}` }
    : { ...base, status: 'fail', detail: `${version} doesn't meet engines ${engines}`, fix: `install Node ${engines} (nvm install 24, or nodejs.org)` };
}

export function gib(bytes: number): string {
  return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
}

/** Free space under the target dir: under 20 GB is a warning, never a block (the build may still fit). */
export function diskCheck(freeBytes: number | undefined, where: string): Check {
  const base = { id: 'disk', title: 'Free disk' };
  if (freeBytes === undefined) return { ...base, status: 'info', detail: `couldn't read free space under ${where}` };
  return freeBytes < DISK_WARN_BYTES
    ? { ...base, status: 'warn', detail: `${gib(freeBytes)} free under ${where} (under ${gib(DISK_WARN_BYTES)})`, fix: 'free space, or `cargo clean` an old target dir (a workspace build takes ~10 GB)' }
    : { ...base, status: 'ok', detail: `${gib(freeBytes)} free under ${where}` };
}

/** One shared target dir per machine (AGENTS.md 'Git and CI'): unset means each worktree builds everything again. */
export function targetDirCheck(env: string | undefined, fallback: string): Check {
  const base = { id: 'target-dir', title: 'CARGO_TARGET_DIR' };
  return env
    ? { ...base, status: 'ok', detail: `${env} (shared by every checkout)` }
    : { ...base, status: 'info', detail: `unset: builds go to ${fallback}, one per checkout`, fix: 'export CARGO_TARGET_DIR=~/.cargo-target in your shell profile (one shared dir per machine)' };
}

interface LockEntry {
  version?: unknown;
  link?: unknown;
  resolved?: unknown;
  optional?: unknown;
  devOptional?: unknown;
  peer?: unknown;
}
export interface Lockfile {
  packages?: Record<string, LockEntry>;
}

/** What `npm install` would change: node_modules entries of the lockfile missing from npm's hidden lockfile, or at another version. */
export function lockDrift(lock: Lockfile, installed: Lockfile | undefined): { missing: string[]; changed: string[] } {
  const want = lock.packages ?? {};
  const have = installed?.packages ?? {};
  const missing: string[] = [];
  const changed: string[] = [];
  for (const [key, e] of Object.entries(want)) {
    if (!key.startsWith('node_modules/')) continue;
    const h = have[key];
    if (e.link === true) {
      // a workspace: a new one pulled without `npm install` isn't linked yet
      if (h?.link !== true || h.resolved !== e.resolved) missing.push(key.slice('node_modules/'.length));
    } else if (!h) {
      // platform builds (esbuild, rollup) and optional peers are skipped on other machines
      if (e.optional !== true && e.devOptional !== true && e.peer !== true) missing.push(key.slice('node_modules/'.length));
    } else if (typeof e.version === 'string' && h.version !== e.version) changed.push(key.slice('node_modules/'.length));
  }
  return { missing, changed };
}

export function depsCheck(hasNodeModules: boolean, drift: { missing: string[]; changed: string[] } | undefined): Check {
  const base = { id: 'npm-deps', title: 'npm deps' };
  if (!hasNodeModules) return { ...base, status: 'fail', detail: 'node_modules is missing', fix: 'npm install' };
  if (!drift) return { ...base, status: 'warn', detail: "node_modules has no .package-lock.json, so it can't be compared with package-lock.json", fix: 'npm install' };
  const n = drift.missing.length + drift.changed.length;
  if (!n) return { ...base, status: 'ok', detail: 'node_modules matches package-lock.json' };
  const list = [...drift.missing.map((p) => `${p} missing`), ...drift.changed.map((p) => `${p} at another version`)];
  return { ...base, status: 'fail', detail: `${n} out of sync with package-lock.json: ${list.slice(0, 3).join(', ')}${n > 3 ? `, +${n - 3} more` : ''}`, fix: 'npm install' };
}

export interface ToolchainFacts {
  /** rustup's version line, undefined when rustup isn't on PATH. */
  rustup: string | undefined;
  /** rust-toolchain.toml's channel. */
  pinned: string;
  /** `rustup toolchain list` names, e.g. `1.100.0-aarch64-apple-darwin`. */
  installed: readonly string[];
  /** $RUSTUP_TOOLCHAIN. */
  override: string | undefined;
  /** The override toolchain's own rustc version (`1.100.0`), read without the rustup proxy; undefined when unknown. */
  overrideVersion?: string | undefined;
  /** Components installed on the pinned toolchain (undefined when it isn't installed). */
  components: readonly string[] | undefined;
  /** rust-toolchain.toml's components. */
  wantComponents: readonly string[];
}

/** rustup + the pinned toolchain, its components, and no RUSTUP_TOOLCHAIN overriding it. */
export function toolchainChecks(f: ToolchainFacts): Check[] {
  if (!f.rustup) {
    return [{ id: 'rustup', title: 'rustup', status: 'fail', detail: 'rustup is not on PATH (a distro or brew cargo ignores rust-toolchain.toml)', fix: 'install rustup: https://rustup.rs' }];
  }
  const out: Check[] = [{ id: 'rustup', title: 'rustup', status: 'ok', detail: f.rustup }];
  const pinnedName = (n: string): boolean => n === f.pinned || n.startsWith(`${f.pinned}-`);
  const has = f.installed.some(pinnedName);
  out.push(
    has
      ? { id: 'toolchain', title: 'Rust toolchain', status: 'ok', detail: `${f.pinned} (rust-toolchain.toml) is installed` }
      : { id: 'toolchain', title: 'Rust toolchain', status: 'warn', detail: `${f.pinned} (rust-toolchain.toml) is not installed; the first cargo call downloads it`, fix: `rustup toolchain install ${f.pinned} --profile minimal --component ${f.wantComponents.join(',')}` },
  );
  if (f.override && !pinnedName(f.override)) {
    const same = f.overrideVersion !== undefined && compareVersions(f.overrideVersion, f.pinned) === 0 && parseVersion(f.pinned) !== undefined;
    // rustup sets RUSTUP_TOOLCHAIN for every process it proxies, so anything started under `cargo run` inherits it
    out.push(
      same
        ? { id: 'rustup-override', title: 'RUSTUP_TOOLCHAIN', status: 'warn', detail: `RUSTUP_TOOLCHAIN=${f.override} overrides rust-toolchain.toml, but it is ${f.overrideVersion} like the pin (often inherited from a parent cargo or rustup)`, fix: 'unset RUSTUP_TOOLCHAIN before cargo, so a later stable doesn\'t drift from CI' }
        : { id: 'rustup-override', title: 'RUSTUP_TOOLCHAIN', status: 'fail', detail: `RUSTUP_TOOLCHAIN=${f.override}${f.overrideVersion ? ` (${f.overrideVersion})` : ''} overrides rust-toolchain.toml (${f.pinned}): rustfmt and clippy won't match CI`, fix: 'unset RUSTUP_TOOLCHAIN (in your shell profile, or the process that started this one)' },
    );
  }
  if (has && f.components) {
    const lacking = f.wantComponents.filter((c) => !f.components?.some((x) => x === c || x.startsWith(`${c}-`)));
    out.push(
      lacking.length
        ? { id: 'components', title: 'clippy + rustfmt', status: 'warn', detail: `${lacking.join(', ')} not installed on ${f.pinned}`, fix: `rustup component add ${lacking.join(' ')} --toolchain ${f.pinned}` }
        : { id: 'components', title: 'clippy + rustfmt', status: 'ok', detail: `${f.wantComponents.join(', ')} on ${f.pinned}` },
    );
  }
  return out;
}

/** rust-toolchain.toml's channel and components (the file is tiny and flat; no TOML parser needed). */
export function parseToolchainToml(text: string): { channel: string | undefined; components: string[] } {
  const channel = /^\s*channel\s*=\s*"([^"]+)"/m.exec(text)?.[1];
  const list = /^\s*components\s*=\s*\[([^\]]*)\]/m.exec(text)?.[1] ?? '';
  return { channel, components: [...list.matchAll(/"([^"]+)"/g)].map((m) => m[1] ?? '').filter(Boolean) };
}

export function monoflowCheck(binary: boolean, wrapper: boolean): Check {
  const base = { id: 'monoflow', title: 'monoflow' };
  if (binary && wrapper) return { ...base, status: 'ok', detail: '.agents/bin/monoflow is installed' };
  const what = !binary ? '.agents/bin/monoflow-bin is missing' : '.agents/bin/monoflow (the wrapper) is missing';
  return { ...base, status: 'warn', detail: `${what}: npm test falls back to per-package runs, and graph impact isn't available`, fix: 'node tools/setup-monoflow.ts (or --build to build the pinned commit)' };
}

/** Git hooks: monoflow's shield hooks (or its git shim) must never be installed (plan §9.4.3); other hooks are listed. */
export function hooksCheck(monoflowStrays: readonly string[], otherHooks: readonly string[], hooksPath: string | undefined): Check {
  const base = { id: 'git-hooks', title: 'git hooks' };
  if (monoflowStrays.length) {
    return { ...base, status: 'fail', detail: `monoflow init left: ${monoflowStrays.join(', ')} (they rewrite shared worktrees)`, fix: 'delete them; never run `monoflow init` (node tools/setup-monoflow.ts installs only the binary and wrapper)' };
  }
  const where = hooksPath ? ` (core.hooksPath=${hooksPath})` : '';
  return otherHooks.length
    ? { ...base, status: 'info', detail: `active hooks${where}: ${otherHooks.join(', ')}; wisp installs none` }
    : { ...base, status: 'ok', detail: `none installed${where}; wisp needs none, and never monoflow's` };
}

export function webBuildCheck(present: boolean): Check {
  return present
    ? { id: 'web-build', title: 'web build', status: 'ok', detail: 'apps/web/dist is built (wisp serve serves it)' }
    : { id: 'web-build', title: 'web build', status: 'warn', detail: "apps/web/dist is missing: `wisp serve` from this checkout has no UI (npm run dev doesn't need it)", fix: 'npm run build' };
}

export function fakeAgentsCheck(found: readonly string[], want: readonly string[], dir: string): Check {
  const lacking = want.filter((w) => !found.includes(w));
  return lacking.length
    ? { id: 'fake-agents', title: 'fake agents', status: 'warn', detail: `${lacking.join(', ')} not built in ${dir}: crates/wisp serve_* tests build their own on first use; build them before running wisp-testkit's or a serve test by hand`, fix: 'cargo build -p wisp-testkit --bins' }
    : { id: 'fake-agents', title: 'fake agents', status: 'ok', detail: `${want.join(', ')} built in ${dir}` };
}

export type Os = 'darwin' | 'linux' | 'win32' | 'other';

/** The Tauri prerequisites for `npm run dev:desktop`. On macOS the CLT also give cargo its linker, so missing blocks. */
export function desktopCheck(os: Os, present: boolean | undefined): Check {
  const base = { id: 'desktop', title: 'desktop (Tauri)' };
  const need: Record<Os, { what: string; fix: string }> = {
    darwin: { what: 'Xcode Command Line Tools', fix: 'xcode-select --install' },
    linux: { what: 'webkit2gtk-4.1', fix: 'sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev libayatana-appindicator3-dev librsvg2-dev (or your distro\'s equivalent)' },
    win32: { what: 'WebView2 runtime', fix: 'install the WebView2 Evergreen runtime: https://developer.microsoft.com/microsoft-edge/webview2/' },
    other: { what: 'the Tauri prerequisites', fix: 'see https://tauri.app/start/prerequisites/' },
  };
  const n = need[os];
  if (present === undefined) return { ...base, status: 'info', detail: `couldn't tell whether ${n.what} is installed (needed for npm run dev:desktop)`, fix: n.fix };
  if (present) return { ...base, status: 'ok', detail: `${n.what} found (npm run dev:desktop)` };
  return os === 'darwin'
    ? { ...base, status: 'fail', detail: `${n.what} missing: cargo has no linker, and npm run dev:desktop can't build`, fix: n.fix }
    : { ...base, status: 'warn', detail: `${n.what} missing: only npm run dev:desktop needs it`, fix: n.fix };
}

/** Full Xcode, needed only for the iOS app (mobile/ios); macOS only. */
export function xcodeCheck(os: Os, hasXcode: boolean): Check | undefined {
  if (os !== 'darwin') return undefined;
  return hasXcode
    ? { id: 'xcode', title: 'Xcode', status: 'ok', detail: 'Xcode.app found (mobile/ios)' }
    : { id: 'xcode', title: 'Xcode', status: 'info', detail: 'Xcode.app not found: needed only for the iOS app (mobile/ios)', fix: 'install Xcode from the App Store when you work on mobile/ios' };
}

export function devPortCheck(busy: boolean): Check {
  return busy
    ? { id: 'dev-port', title: `port ${DEV_PORT}`, status: 'warn', detail: `${DEV_PORT} is in use: npm run dev will take the next free port (a dev server already running?)`, fix: `lsof -i tcp:${DEV_PORT} -sTCP:LISTEN to see what holds it` }
    : { id: 'dev-port', title: `port ${DEV_PORT}`, status: 'ok', detail: `${DEV_PORT} is free for npm run dev` };
}

export function installedWispCheck(busy: boolean): Check {
  return busy
    ? { id: 'installed-wisp', title: `port ${INSTALLED_PORT}`, status: 'info', detail: `something listens on ${INSTALLED_PORT} (an installed wisp): left alone; dev runs never use ${INSTALLED_PORT} or ~/.wisp` }
    : { id: 'installed-wisp', title: `port ${INSTALLED_PORT}`, status: 'ok', detail: `no installed wisp running on ${INSTALLED_PORT}` };
}

/** 1 when any check blocks, else 0: advice never fails the run. */
export function exitCode(checks: readonly Check[]): 0 | 1 {
  return checks.some((c) => c.status === 'fail') ? 1 : 0;
}

const MARK: Record<Status, string> = { ok: 'ok  ', warn: 'warn', fail: 'FAIL', info: 'note' };

/** The human report: one line per check, its fix indented under it, and a summary line. */
export function format(checks: readonly Check[], ms: number): string {
  const w = Math.max(...checks.map((c) => c.title.length));
  const lines: string[] = [];
  for (const c of checks) {
    lines.push(`${MARK[c.status]}  ${c.title.padEnd(w)}  ${c.detail}`);
    if (c.fix && c.status !== 'ok') lines.push(`      ${' '.repeat(w)}  → ${c.fix}`);
  }
  const fails = checks.filter((c) => c.status === 'fail').length;
  const warns = checks.filter((c) => c.status === 'warn').length;
  lines.push('');
  lines.push(fails ? `${fails} blocking, ${warns} advice: fix the FAIL lines first (${ms} ms)` : warns ? `ready, with ${warns} advice (${ms} ms)` : `ready (${ms} ms)`);
  return `${lines.join('\n')}\n`;
}
