import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test, { after } from 'node:test';
import { discover } from '../check/core/workspace.ts';
import { cargoFailures, tscFailures, vitestByPackage } from '../ci-logs.ts';
import { covers, plan as verifyPlan, readProgram } from '../ci-plan.ts';
import { candidates, combine, manifestDirs, merge, monoflowPiece, WRAPPER_ENV } from '../ci-verify.ts';
import { affectedSet, parseImpact, scorePr, verdict, type PrScore } from '../g14.ts';
import { clippedHint, fenceFor, markdown, packageFolders, readVerify, slowest } from '../monoflow-report.ts';
import { MONOFLOW_COMMIT, WRAPPER, WRAPPER_SCRIPT, strays, writeWrapper } from '../setup-monoflow.ts';
import { dependencyOrder, plan } from '../test.ts';
import { PROGRAMS } from '../typecheck.ts';

const roots: string[] = [];
after(() => {
  for (const r of roots) rmSync(r, { recursive: true, force: true });
});
function dir(files: Record<string, string>): string {
  const root = mkdtempSync(join(tmpdir(), 'wisp-tools-'));
  roots.push(root);
  for (const [p, body] of Object.entries(files)) {
    mkdirSync(join(root, p, '..'), { recursive: true });
    writeFileSync(join(root, p), body);
  }
  return root;
}

const ws = (name: string, deps: string[] = [], test = true): string => JSON.stringify({ name, dependencies: Object.fromEntries(deps.map((d) => [d, '*'])), scripts: test ? { test: 'node -e 0' } : {} });

test('test.ts: workspaces run in dependency order, ties by name, packages without tests skipped', () => {
  const root = dir({
    'package.json': JSON.stringify({ workspaces: ['packages/*', 'apps/*'] }),
    'packages/kernel/package.json': ws('@w/kernel', ['@w/protocol']),
    'packages/protocol/package.json': ws('@w/protocol'),
    'packages/a-tokens/package.json': ws('@w/tokens'),
    'packages/docs/package.json': ws('@w/docs', [], false),
    'apps/server/package.json': ws('server', ['@w/kernel', 'left-pad']),
  });
  assert.deepEqual(dependencyOrder(discover(root)).map((p) => p.name), ['@w/docs', '@w/protocol', '@w/tokens', '@w/kernel', 'server']);
  const steps = plan(root, { monoflow: false }).map((s) => s.name);
  assert.deepEqual(steps, ['tools', '@w/protocol', '@w/tokens', '@w/kernel', 'server']);
});

test('test.ts: with monoflow installed, packages run through verify --all --test', () => {
  // installed = the sh wrapper on macOS/Linux, the binary on Windows (setup-monoflow writes both; Node can't spawn sh there)
  const root = dir({ 'package.json': JSON.stringify({ workspaces: [] }), '.agents/bin/monoflow': '', '.agents/bin/monoflow-bin.exe': '' });
  for (const [platform, bin] of [['linux', 'monoflow'], ['darwin', 'monoflow'], ['win32', 'monoflow-bin.exe']] as const) {
    const steps = plan(root, { monoflow: true, platform });
    assert.deepEqual(steps.map((s) => s.args.join(' ')), ['--test --test-reporter=dot tools/**/*.test.ts', 'verify --all --test'], platform);
    assert.equal(steps[1]?.cmd, join(root, '.agents', 'bin', bin), platform);
    assert.deepEqual(plan(root, { monoflow: false, platform }).length, 1);
  }
  const wrapperOnly = dir({ 'package.json': JSON.stringify({ workspaces: [] }), '.agents/bin/monoflow': '' });
  assert.equal(plan(wrapperOnly, { monoflow: true, platform: 'win32' }).length, 1, 'on Windows the sh wrapper alone is no monoflow: the plain npm fallback');
});

test('test.ts: dependency cycles still run every package', () => {
  const root = dir({ 'package.json': JSON.stringify({ workspaces: ['packages/*'] }), 'packages/a/package.json': ws('a', ['b']), 'packages/b/package.json': ws('b', ['a']) });
  assert.deepEqual(dependencyOrder(discover(root)).map((p) => p.name), ['a', 'b']);
});

test('monoflow-report: both versions of verify --json read the same', () => {
  const pin = readVerify({ status: 'failed', duration_ms: 3000, total_targets: 2, affected_packages: ['b', 'a'], results: [{ target: 'a', status: 'passed', duration_ms: 1000 }, { target: 'b', status: 'failed', duration_ms: 2000 }] });
  assert.deepEqual(pin, { status: 'failed', durationMs: 3000, affected: ['a', 'b'], failed: ['b'], durations: { a: 1, b: 2 }, total: 2, details: [] });
  const later = readVerify({ status: 'failed', duration_ms: 10, total_targets: 3, failed: { c: 2 }, errors: [{ file: 'packages/d', targets: ['d'], message: 'tsc: 2 errors' }] });
  assert.deepEqual(later.failed, ['c', 'd']);
  assert.deepEqual(later.details, ['packages/d: tsc: 2 errors']);
  assert.deepEqual(readVerify('garbage').status, 'unknown');
  const md = markdown('PR', pin, { affected_packages: ['a'], unmapped_files: ['README.md'] });
  assert.match(md, /Failed: `b`/);
  assert.match(md, /1 changed files outside every package/);
});

test('monoflow-report: a clipped execution_failure says the test is cut out and how to see it, or names it', () => {
  // the message PR CI got for wisp-grid (1fecc6c5): the binaries before the failing one, nothing after
  const ci = '…work/wisp/wisp/target/debug/build/wisp-grid/16/out/wisp_grid-16)\nrunning 0 tests\ntest result: ok. 0 passed; 0 failed\n     Running unittests src/bin/wisp-grid-hostd.rs (…)\nrunning 0 tests\ntest result: ok…';
  const r = readVerify({ status: 'failed', failed: { 'wisp-grid': 1 }, errors: [{ file: 'apps/grid', code: 'execution_failure', targets: ['wisp-grid'], message: ci }] });
  assert.equal(r.details.length, 2);
  assert.match(r.details[1] ?? '', /^apps\/grid: monoflow cut the failing test out .*to see it: cargo test -p wisp-grid --no-fail-fast$/);
  assert.match(markdown('PR', r), /What failed[\s\S]*cargo test -p wisp-grid/);
  const panic = "thread 'hostd_runs_a_box_through_boxd' panicked at apps/grid/tests/boxd_driver.rs:105:5:\ntest hostd_runs_a_box_through_boxd ... FAILED";
  assert.equal(
    clippedHint(panic, ['x'], 'packages/x', () => false),
    "failing: thread 'hostd_runs_a_box_through_boxd' panicked at apps/grid/tests/boxd_driver.rs:105:5: | test hostd_runs_a_box_through_boxd ... FAILED; to see it: the test script in packages/x",
  );
});

test('monoflow-report: an execution_failure with no file resolves its target to the crate folder', () => {
  const r = readVerify({ status: 'failed', errors: [{ code: 'execution_failure', targets: ['wisp-grid'], message: 'test result: ok…' }] }, (t) => (t === 'wisp-grid' ? 'apps/grid' : undefined));
  assert.match(r.details[1] ?? '', /^wisp-grid: .*to see it: cargo test -p wisp-grid --no-fail-fast$/);
  // the real resolver, from cargo metadata and the npm workspaces of this repo
  const folders = packageFolders();
  assert.equal(folders.get('wisp'), 'crates/wisp');
  assert.equal(folders.get('@wisp/ui'), 'packages/ui');
  // a target nobody knows keeps the target names as the folder (and so the test script hint)
  const unknown = readVerify({ errors: [{ code: 'execution_failure', targets: ['nope'], message: '' }] }, () => undefined);
  assert.match(unknown.details.at(-1) ?? '', /the test script in nope$/);
});

test('monoflow-report: a ``` inside an error message does not close the What failed block', () => {
  const r = readVerify({ errors: [{ file: 'packages/d', targets: ['d'], message: 'expected:\n```ts\nx\n```\nand ````` too' }] });
  const md = markdown('PR', r);
  const fence = fenceFor(r.details);
  assert.equal(fence, '``````');
  assert.ok(md.includes(`${fence}\npackages/d: expected:`));
  assert.equal(md.split('\n').filter((l) => l === fence).length, 2);
  assert.equal(fenceFor(['plain']), '```');
});

test('monoflow-report: ✖ and FAIL count only at the start of a line', () => {
  const msg = ['✖ adds two numbers (1.2ms)', ' FAIL  src/a.test.ts > a', 'note: no ✖ here', 'AUTOFAIL x', 'skipped FAIL marker'].join('\n');
  assert.equal(clippedHint(msg, ['x'], 'packages/x', () => false), 'failing: ✖ adds two numbers (1.2ms) | FAIL  src/a.test.ts > a; to see it: the test script in packages/x');
});

test('g14: impact in either shape; affected set is owners plus dependents; unmapped files listed', () => {
  assert.deepEqual(parseImpact({ owning_package: 'p', transitive_dependents: ['q'] }), { owner: 'p', dependents: ['q'] });
  assert.deepEqual(parseImpact({ package: 'p', dependent_packages: ['r'] }), { owner: 'p', dependents: ['r'] });
  assert.equal(parseImpact({ file: 'x', impact: 'none (untracked/root file)' }), undefined);
  const asked: string[] = [];
  const impact = (f: string) => {
    asked.push(f);
    return f.startsWith('packages/p/') ? { owner: 'p', dependents: ['q', 'r'] } : f.startsWith('plugins/') ? { owner: 'x', dependents: [] } : undefined;
  };
  const a = affectedSet(['packages/p/src/a.ts', 'packages/p/src/b.ts', 'plugins/x/api/i.ts', 'README.md', 'docs/a.md'], impact);
  assert.deepEqual(a, { affected_packages: ['p', 'q', 'r', 'x'], unmapped_files: ['README.md', 'docs/a.md'] });
  assert.equal(asked.length, 4); // packages/p/src asked once
});

test('g14: a failure outside the affected set is a miss; time saved comes from the full run', () => {
  const full = { status: 'failed', durationMs: 0, affected: [], failed: ['q'], durations: { p: 1, q: 1, r: 2 }, total: 3, details: [] };
  assert.deepEqual(scorePr({ number: 1, affected: ['p', 'q'], unmapped: 0, full }), { number: 1, misses: [], saved: 0.5 });
  assert.deepEqual(scorePr({ number: 2, affected: ['p'], unmapped: 0, full }), { number: 2, misses: ['q'], saved: 0.75 });
  assert.equal(scorePr({ number: 3, affected: [], unmapped: 0, full: undefined }), undefined);
});

test('g14: the gate needs 200 PRs, zero misses and half the time saved', () => {
  const ok = (n: number, saved = 0.6): PrScore[] => Array.from({ length: n }, (_, i) => ({ number: i, misses: [], saved }));
  assert.equal(verdict(ok(200)).pass, true);
  assert.equal(verdict(ok(199)).pass, false);
  assert.equal(verdict(ok(200, 0.4)).pass, false);
  const one = [...ok(199), { number: 999, misses: ['@wisp/store'], saved: 0.9 }];
  const v = verdict(one);
  assert.equal(v.pass, false);
  assert.match(v.reasons.join(' '), /#999/);
});

test('setup-monoflow: the pin is a full sha; strays from monoflow init are found, a clean repo has none', () => {
  assert.match(MONOFLOW_COMMIT, /^[0-9a-f]{40}$/);
  const root = dir({ 'README.md': '' });
  execFileSync('git', ['init', '-q'], { cwd: root });
  assert.deepEqual(strays(root), []);
  mkdirSync(join(root, '.agents/bin'), { recursive: true });
  writeFileSync(join(root, '.agents/bin/git'), '#!/bin/sh\n');
  writeFileSync(join(root, '.git/hooks/pre-push'), '#!/bin/sh\n# MonoFlow git shield\n');
  writeFileSync(join(root, '.git/hooks/post-merge'), '#!/bin/sh\necho mine\n');
  mkdirSync(join(root, '.opencode'));
  const found = strays(root).join('\n');
  assert.match(found, /\.agents\/bin\/git/);
  assert.match(found, /pre-push/);
  assert.match(found, /\.opencode/);
  assert.doesNotMatch(found, /post-merge/);
});

test('setup-monoflow: the wrapper refuses what rewrites a shared tree and execs the binary beside it; written once', () => {
  for (const refused of ['"checkpoint revert"*', '"commit "*', '"outer "*', '"ship "*', '"start "*', '"sync "*', '"push "*']) assert.ok(WRAPPER_SCRIPT.includes(refused), refused);
  assert.match(WRAPPER_SCRIPT, /^#!\/bin\/sh\n/);
  assert.match(WRAPPER_SCRIPT, /exec "\$\(dirname "\$0"\)\/monoflow-bin" "\$@"\n$/);
  const root = mkdtempSync(join(tmpdir(), 'wisp-mf-'));
  try {
    assert.equal(writeWrapper(root), true);
    assert.equal(readFileSync(join(root, WRAPPER), 'utf8'), WRAPPER_SCRIPT);
    assert.equal(writeWrapper(root), false, 'up to date: nothing written');
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('ci-verify: the crates a --no-fail-fast cargo run names (failed targets, builds that broke), through colours and timestamps', () => {
  const log = [
    '2026-11-15T02:40:00.0000000Z \x1b[1m\x1b[91merror\x1b[0m: could not compile `wisp-store` (test "store") due to 2 previous errors',
    'test a ... ok',
    'error: 3 targets failed:',
    '    `-p wisp-runtime --test owed`',
    '    `-p wisp-runtime --lib`',
    '2026-11-15T02:41:00.0000000Z     `-p wisp --test serve_desktop`',
    'error: test failed, to rerun pass `-p other --lib`',
  ].join('\r\n');
  const f = cargoFailures(log);
  assert.deepEqual([...f.keys()].sort(), ['wisp', 'wisp-runtime', 'wisp-store']);
  assert.deepEqual(f.get('wisp-runtime'), ['--test owed failed', '--lib failed']);
  assert.deepEqual(f.get('wisp-store'), ['doesn\'t compile (test "store")']);
  assert.equal(cargoFailures('test result: ok. 3 passed\n').size, 0, 'a green run names nothing');
});

test('ci-verify: monoflow is asked about every manifest folder but the workspace members (unless they hold another manifest)', () => {
  const files = ['Cargo.toml', 'package.json', 'crates/a/Cargo.toml', 'crates/a/src/lib.rs', 'both/Cargo.toml', 'both/package.json', 'spikes/x/Cargo.toml', 'apps/web/package.json', 'sdk/python/pyproject.toml', 'x/node_modules/y/package.json', 'crates/a/tests/fixtures/package.json'];
  assert.deepEqual(manifestDirs(files), ['.', 'apps/web', 'both', 'crates/a', 'sdk/python', 'spikes/x']);
  assert.deepEqual(candidates(files, new Set(['crates/a', 'both'])), ['apps/web', 'both', 'sdk/python', 'spikes/x']);
});

test("ci-verify: one result in monoflow's shape: counts, failures from monoflow and cargo, slowest first", () => {
  const parts = [
    { target: '@w/ui', path: 'packages/ui', ms: 9000, raw: { status: 'passed', duration_ms: 8000, cached_targets: 0 } },
    { target: '@w/web', path: 'apps/web', ms: 20000, raw: { status: 'failed', duration_ms: 19000, failed: { '@w/web': 1 }, errors: [{ code: 'test-failure', file: 'apps/web/test/a.test.ts', message: 'boom', targets: ['@w/web'] }] } },
    { target: 'bench', path: 'spikes/b', ms: 500, raw: { status: 'failed', error: 'monoflow printed no JSON: x' } },
  ];
  const r = merge(parts, { ran: true, failures: new Map([['wisp-runtime', ['--test owed failed']]]), crates: 10 }, 30000);
  assert.equal(r['status'], 'failed');
  assert.equal(r['total_targets'], 13);
  assert.equal(r['failed_targets'], 3);
  assert.equal(r['passed_targets'], 10);
  assert.deepEqual(Object.keys(r['failed'] as object).sort(), ['@w/web', 'bench', 'wisp-runtime']);
  const v = readVerify(r, () => undefined);
  assert.deepEqual(v.failed, ['@w/web', 'bench', 'wisp-runtime']);
  assert.ok(v.details.some((d) => d.includes('boom')) && v.details.some((d) => d.includes('no JSON')) && v.details.some((d) => d.includes('--test owed')));
  assert.deepEqual(slowest(v, 2), [['@w/web', 19], ['@w/ui', 8]]);
  assert.match(markdown('t', v), /Slowest: `@w\/web` 19 s, `@w\/ui` 8 s/);
  // crates/wisp and sdk/python are both `wisp`: the crate's failure is keyed by its folder, never credited to sdk/python
  const py = [{ target: 'wisp', path: 'sdk/python', ms: 4000, raw: { status: 'passed', duration_ms: 4000 } }];
  const clash = merge(py, { ran: true, failures: new Map([['wisp', ['--test serve_desktop failed']]]), crates: 5, dirs: new Map([['wisp', 'crates/wisp']]) }, 1);
  assert.deepEqual(Object.keys(clash['failed'] as object), ['crates/wisp']);
  assert.equal(readVerify(clash, () => undefined).durations['wisp'], 4, "sdk/python keeps its time");
  assert.equal(clash['passed_targets'], 5);
  const green = merge(parts.slice(0, 1), { ran: false, failures: new Map(), crates: 10 }, 1);
  assert.equal(green['status'], 'passed');
  assert.equal(green['passed_targets'], 1, 'crates without a cargo test result are not counted as passed');
  assert.match(String(green['rust_workspace']), /didn't run/);
});

test('ci-verify: calls monoflow-bin with the environment the wrapper sets', () => {
  for (const [k, v] of Object.entries(WRAPPER_ENV)) assert.match(WRAPPER_SCRIPT, new RegExp(`export [^\\n]*\\b${k}=${v}\\b`));
  assert.deepEqual([...WRAPPER_SCRIPT.matchAll(/\b(MONOFLOW_\w+)=/g)].map((m) => m[1]).sort(), Object.keys(WRAPPER_ENV).sort());
});

test('ci-verify: a package splits only when monoflow would run nothing but its typecheck and test scripts', () => {
  const yes = { vitestConfig: true, rootLint: false, covered: (c: string): boolean => c === 'tsconfig.json' || c === 'tsconfig.dev.json' };
  const ui = { test: 'vitest run --passWithNoTests', typecheck: 'tsc -p tsconfig.json', dev: 'vite' };
  assert.deepEqual(verifyPlan(ui, yes), { kind: 'split', typecheck: 'covered', test: 'vitest', needsTests: false, configs: ['tsconfig.json'] });
  const web = verifyPlan({ test: 'vitest run', typecheck: 'tsc -p tsconfig.json && tsc -p tsconfig.dev.json' }, yes);
  assert.deepEqual(web, { kind: 'split', typecheck: 'covered', test: 'vitest', needsTests: true, configs: ['tsconfig.json', 'tsconfig.dev.json'] });
  // node --test: its typecheck is covered, its tests go through monoflow
  assert.deepEqual(verifyPlan({ test: 'node --test "test/**/*.test.ts"', typecheck: 'tsc -p tsconfig.json' }, { ...yes, vitestConfig: false }), { kind: 'split', typecheck: 'covered', test: 'monoflow', needsTests: true, configs: ['tsconfig.json'] });
  // one config npm run typecheck doesn't hold: the typecheck goes through monoflow, the tests still join the one run
  assert.deepEqual(verifyPlan({ test: 'vitest run', typecheck: 'tsc -p tsconfig.json && tsc -p tsconfig.other.json' }, yes), { kind: 'split', typecheck: 'monoflow', test: 'vitest', needsTests: true, configs: ['tsconfig.json', 'tsconfig.other.json'] });
  // anything else is monoflow's whole verify, as before
  const cases: [Record<string, string> | undefined, typeof yes, string][] = [
    [undefined, yes, 'sdk/python, a spike'],
    [{ ...ui, lint: 'eslint .' }, yes, 'a lint script: monoflow runs it'],
    [ui, { ...yes, rootLint: true }, 'a root eslint config gives every package a lint command'],
    [{ test: 'vitest run' }, yes, 'no typecheck script: monoflow may type-check through the root tsconfig'],
    [{ typecheck: 'tsc -p tsconfig.json' }, yes, 'no test script'],
    [{ test: 'vitest run --coverage', typecheck: 'tsc --noEmit' }, yes, 'neither in a form ci-verify runs'],
    [{ test: 'vitest run', typecheck: 'tsc -p tsconfig.json' }, { ...yes, vitestConfig: false, covered: () => false }, 'no vitest.config.ts, typecheck not covered'],
  ];
  for (const [scripts, o, why] of cases) assert.deepEqual(verifyPlan(scripts, o), { kind: 'monoflow' }, why);
});

test("ci-verify: a package's tsconfig is covered only by a program with its options and all of its files", () => {
  const pkg = { files: ['/r/p/src/a.ts', '/r/p/test/a.test.ts'], options: '{"strict":true}' };
  assert.equal(covers(pkg, [undefined, { files: ['/r/p/src/a.ts', '/r/p/test/a.test.ts', '/r/q/b.ts'], options: '{"strict":true}' }]), true);
  assert.equal(covers(pkg, [{ files: ['/r/p/src/a.ts'], options: '{"strict":true}' }]), false, 'a file the program lacks');
  assert.equal(covers(pkg, [{ files: pkg.files, options: '{"strict":true,"types":["node"]}' }]), false, 'other options');
  assert.equal(covers({ files: [], options: '{}' }, [{ files: [], options: '{}' }]), false, "no inputs is tsc's own error");
  assert.equal(covers(undefined, [pkg]), false, "a config that doesn't parse");
  // the repo's own: a plugin UI's config is the web program's; the web app's dev config is only its own program's
  const programs = PROGRAMS.map((p) => readProgram(p));
  assert.equal(covers(readProgram('plugins/goal/ui/tsconfig.json'), programs), true);
  assert.equal(covers(readProgram('packages/tokens/tsconfig.json'), programs), true);
  assert.equal(covers(readProgram('apps/web/tsconfig.dev.json'), programs.filter((_, i) => PROGRAMS[i] !== 'apps/web/tsconfig.dev.json')), false, 'node and vite types: no other program has its options');
});

test("ci-verify: npm run typecheck's errors by file, pretty or not, and whether it finished", () => {
  const log = [
    '> wisp-workspace@ typecheck',
    '── tsconfig.web.json',
    "\x1b[96mplugins/goal/ui/src/index.ts\x1b[0m:\x1b[93m3\x1b[0m:\x1b[93m5\x1b[0m - \x1b[91merror\x1b[0m\x1b[90m TS2322: \x1b[0mType 'x' is not assignable.",
    "2026-11-15T02:40:00.0000000Z packages\\ui\\src\\a.tsx(10,2): error TS2304: Cannot find name 'y'.",
    "error TS5083: Cannot read file '/x/tsconfig.json'.",
    'typecheck: 1 of 4 programs failed (9.0s)',
  ].join('\r\n');
  const r = tscFailures(log);
  assert.equal(r.done, true);
  assert.deepEqual([...r.files.keys()], ['plugins/goal/ui/src/index.ts', 'packages/ui/src/a.tsx']);
  assert.match(r.files.get('plugins/goal/ui/src/index.ts')?.[0] ?? '', /:3:5 TS2322: Type 'x'/);
  assert.deepEqual(r.other, ["TS5083: Cannot read file '/x/tsconfig.json'."]);
  assert.deepEqual(tscFailures('typecheck: 4 programs, 0 errors (9.2s)\n'), { done: true, files: new Map(), other: [] });
  assert.equal(tscFailures('── tsconfig.json\n').done, false, 'cut off before its summary: not a result');
  assert.equal(tscFailures('typecheck: 1 of 4 programs failed (1s)').other.length, 1, 'a failed program with no error line still fails');
});

test("ci-verify: the one vitest run's report, split by package, Windows paths included", () => {
  const report = {
    success: false,
    testResults: [
      { name: 'D:/a/wisp/wisp/plugins/goal/ui/test/a.test.ts', status: 'passed', startTime: 1000, endTime: 1250, assertionResults: [] },
      { name: 'd:/a/wisp/wisp/plugins/goal/ui/test/b.test.ts', status: 'failed', startTime: 0, endTime: 50, message: '', assertionResults: [{ status: 'passed', fullName: 'ok' }, { status: 'failed', fullName: 'b > breaks', failureMessages: ['\x1b[31mAssertionError: expected 1 to be 2\x1b[39m\n    at b.test.ts:3'] }] },
      { name: 'D:/a/wisp/wisp/packages/ui/test/c.test.tsx', status: 'failed', message: 'Failed to load url ./missing', assertionResults: [] },
      { name: 'D:/a/wisp/wisp/elsewhere/d.test.ts', status: 'failed', message: 'boom', assertionResults: [] },
    ],
  };
  const { per, other } = vitestByPackage(report, ['plugins/goal/ui', 'packages/ui', 'plugins/later/ui'], 'D:\\a\\wisp\\wisp');
  assert.deepEqual(per.get('plugins/goal/ui'), { files: 2, ms: 300, errors: [{ code: 'test-failure', file: 'plugins/goal/ui/test/b.test.ts', message: 'b > breaks: AssertionError: expected 1 to be 2', targets: [] }] });
  assert.deepEqual(per.get('packages/ui')?.errors, [{ code: 'test-failure', file: 'packages/ui/test/c.test.tsx', message: 'Failed to load url ./missing', targets: [] }]);
  assert.deepEqual(per.get('plugins/later/ui'), { files: 0, ms: 0, errors: [] });
  assert.deepEqual(other, ['elsewhere/d.test.ts: boom']);
  assert.match(vitestByPackage({ success: false, testResults: [] }, ['a'], '/r').other[0] ?? '', /outside any test file/, 'a red run with no failing file still fails');
  assert.deepEqual(vitestByPackage({ success: true, testResults: [] }, ['a'], '/r').other, []);
});

test('ci-verify: the vitest run fails when vitest exits red with a green report, or reports a file in no package', () => {
  // an unhandled error (a rejected promise a test left behind): vitest exits 1, its JSON report still says success
  const unhandled = vitestByPackage({ success: true, testResults: [{ name: '/r/a/test/x.test.ts', status: 'passed', assertionResults: [] }] }, ['a'], '/r', 1);
  assert.equal(unhandled.per.get('a')?.files, 1);
  assert.match(unhandled.other[0] ?? '', /exit 1\) outside any test file/);
  // a passing file under a root spelled another way (a symlink, the real path) is matched, never dropped unseen
  const real = vitestByPackage({ success: true, testResults: [{ name: '/private/r/a/test/x.test.ts', status: 'passed' }] }, ['a'], ['/r', '/private/r'], 0);
  assert.deepEqual([real.per.get('a')?.files, real.other], [1, []]);
  const lost = vitestByPackage({ success: true, testResults: [{ name: '/elsewhere/a/test/x.test.ts', status: 'passed' }] }, ['a'], '/r', 0);
  assert.deepEqual([lost.per.get('a')?.files, lost.other.length], [0, 1]);
});

test("ci-verify: a split package's checks fold into one monoflow-shaped result, each named by what ran it", () => {
  const tc = { via: 'npm run typecheck', ok: true, ms: 0, errors: [] };
  const vt = { via: 'vitest (one run)', ok: false, ms: 1200, errors: [{ code: 'test-failure', file: 'plugins/goal/ui/test/b.test.ts', message: 'b > breaks', targets: [] }] };
  const goal = combine('@w/goal-ui', 'plugins/goal/ui', [tc, vt]);
  assert.equal(goal.via, 'npm run typecheck + vitest (one run)');
  const later = combine('@w/later-ui', 'plugins/later/ui', [tc, { ...vt, ok: true, errors: [] }]);
  const tokens = combine('@w/tokens', 'packages/tokens', [tc, monoflowPiece({ status: 'passed', duration_ms: 900 }, 1000, 'monoflow --test')]);
  const r = merge([goal, later, tokens], { ran: false, failures: new Map(), crates: 3 }, 5000);
  assert.deepEqual(r['failed'], { '@w/goal-ui': 1 });
  assert.equal(r['passed_targets'], 2);
  assert.deepEqual((r['errors'] as { targets: string[] }[]).map((e) => e.targets), [['@w/goal-ui']]);
  assert.deepEqual((r['results'] as { target: string; via: string; duration_ms: number }[]).map((x) => [x.target, x.duration_ms, x.via]), [
    ['@w/goal-ui', 1200, 'npm run typecheck + vitest (one run)'],
    ['@w/later-ui', 1200, 'npm run typecheck + vitest (one run)'],
    ['@w/tokens', 900, 'npm run typecheck + monoflow --test'],
  ]);
  const v = readVerify(r, () => undefined);
  assert.deepEqual(v.failed, ['@w/goal-ui']);
  assert.ok(v.details.some((d) => d.includes('b > breaks')));
  // a monoflow call that printed no JSON still fails, with its error
  const broken = monoflowPiece({ status: 'failed', error: 'monoflow printed no JSON: x' }, 10, 'monoflow --test');
  assert.deepEqual([broken.ok, broken.ms, (broken.errors[0] as { message: string }).message], [false, 10, 'monoflow printed no JSON: x']);
});
