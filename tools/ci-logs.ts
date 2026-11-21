// What the CI steps before tools/ci-verify.ts printed, read back per package (ADR-0069, ADR-0072): `cargo test
// --no-fail-fast`'s failing crates, `npm run typecheck`'s errors by file, and the one vitest run's JSON report by
// package folder. Pure: text in, maps out, through colours, GitHub's timestamps and Windows paths.

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v);
const num = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? v : 0);
export const stripAnsi = (s: string): string => s.replace(/\x1b\[[0-9;]*[A-Za-z]/g, '');
const GH_STAMP = /^\d{4}-\d\d-\d\dT[\d:.]+Z /;
export const slash = (p: string): string => p.replaceAll('\\', '/');

/** The crates whose tests failed or didn't build, from `cargo test --no-fail-fast`'s output (colours and all). */
export function cargoFailures(log: string): Map<string, string[]> {
  const out = new Map<string, string[]>();
  const add = (crate: string, what: string): void => {
    const l = out.get(crate) ?? [];
    if (!l.includes(what)) l.push(what);
    out.set(crate, l);
  };
  const lines = stripAnsi(log).split(/\r?\n/);
  let inList = false;
  for (const raw of lines) {
    const line = raw.replace(GH_STAMP, ''); // a GitHub log's timestamps
    const built = /error: could not compile `([^`]+)`(?: \(([^)]+)\))?/.exec(line);
    if (built?.[1]) add(built[1], `doesn't compile${built[2] ? ` (${built[2]})` : ''}`);
    if (/^error: \d+ targets? failed:/.test(line.trim())) {
      inList = true;
      continue;
    }
    if (inList) {
      const t = /^\s+`-p (\S+) ([^`]*)`/.exec(line);
      if (t?.[1]) add(t[1], `${t[2]?.trim() ?? ''} failed`.trim());
      else inList = false;
    }
  }
  return out;
}

/**
 * The errors in `npm run typecheck`'s output (tsc, pretty or not, colours and all), by file (relative to the repo,
 * `/`-separated); `done` when the run got to its summary line, so a missing error means a clean file.
 */
export function tscFailures(log: string): { done: boolean; files: Map<string, string[]>; other: string[] } {
  const files = new Map<string, string[]>();
  const other: string[] = [];
  let done = false;
  let failedPrograms = 0;
  for (const raw of stripAnsi(log).split(/\r?\n/)) {
    const line = raw.replace(GH_STAMP, '').trim();
    const sum = /^typecheck: (?:\d+ programs, 0 errors|(\d+) of \d+ programs failed)/.exec(line);
    if (sum) {
      done = true;
      failedPrograms = Number(sum[1] ?? 0);
      continue;
    }
    const m = /^(.+?):(\d+):(\d+) - error (TS\d+): (.*)$/.exec(line) ?? /^(.+?)\((\d+),(\d+)\): error (TS\d+): (.*)$/.exec(line);
    if (m?.[1]) {
      const file = slash(m[1]).replace(/^\.\//, '');
      const l = files.get(file) ?? [];
      const what = `${file}:${m[2]}:${m[3]} ${m[4]}: ${m[5]}`;
      if (!l.includes(what)) l.push(what);
      files.set(file, l);
      continue;
    }
    const g = /^error (TS\d+): (.*)$/.exec(line);
    if (g) other.push(`${g[1]}: ${g[2]}`);
  }
  if (failedPrograms && files.size === 0 && other.length === 0) other.push(`${failedPrograms} program(s) failed with no error line (see the typecheck output)`);
  return { done, files, other };
}

/**
 * Splits vitest's JSON report (`--reporter=json`) by package folder: test files, time and failures each. `roots` are
 * the repo's spellings (as given and its real path; any separator or drive-letter case), `code` vitest's exit code.
 * What no package owns goes in `other`: a test file outside every folder (a path not understood: never dropped), and
 * a red exit with no failing file (an unhandled error, which vitest's report doesn't count against `success`).
 */
export function vitestByPackage(report: unknown, dirs: readonly string[], roots: string | readonly string[], code = 0): { per: Map<string, { files: number; ms: number; errors: unknown[] }>; other: string[] } {
  const per = new Map(dirs.map((d) => [d, { files: 0, ms: 0, errors: [] as unknown[] }]));
  const other: string[] = [];
  const bases = (typeof roots === 'string' ? [roots] : roots).map((r) => `${slash(r).replace(/\/$/, '')}/`.toLowerCase());
  const byLength = [...dirs].sort((a, b) => b.length - a.length);
  const j = isObj(report) ? report : {};
  let fileFailures = 0;
  for (const r of Array.isArray(j['testResults']) ? j['testResults'] : []) {
    if (!isObj(r) || typeof r['name'] !== 'string') continue;
    const name = slash(r['name']);
    const base = bases.find((b) => name.toLowerCase().startsWith(b));
    const rel = base === undefined ? name : name.slice(base.length);
    const dir = byLength.find((d) => rel.startsWith(`${d}/`));
    const failed = r['status'] === 'failed';
    const messages: string[] = [];
    if (failed) {
      fileFailures++;
      for (const a of Array.isArray(r['assertionResults']) ? r['assertionResults'] : []) {
        if (!isObj(a) || a['status'] !== 'failed') continue;
        const why = Array.isArray(a['failureMessages']) && typeof a['failureMessages'][0] === 'string' ? stripAnsi(a['failureMessages'][0]).split('\n')[0] : '';
        messages.push(`${typeof a['fullName'] === 'string' ? a['fullName'] : 'a test'}: ${why}`);
      }
      if (!messages.length) messages.push(typeof r['message'] === 'string' && r['message'] ? stripAnsi(r['message']).split('\n')[0] ?? 'failed' : 'failed');
    }
    const at = dir === undefined ? undefined : per.get(dir);
    if (!at) {
      other.push(`${rel}: ${failed ? messages.join('; ') : 'a test file in no package of the run (its result is not counted)'}`);
      continue;
    }
    at.files++;
    at.ms += Math.max(0, num(r['endTime']) - num(r['startTime']));
    if (failed) at.errors.push({ code: 'test-failure', file: rel, message: messages.join('\n'), targets: [] });
  }
  if ((j['success'] !== true || code !== 0) && fileFailures === 0) other.push(`vitest failed (exit ${code}) outside any test file (an unhandled error or a project that did not load: see the vitest output above)`);
  return { per, other };
}
