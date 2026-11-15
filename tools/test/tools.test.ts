import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test, { after } from 'node:test';
import { discover } from '../check/core/workspace.ts';
import { candidates, cargoFailures, manifestDirs, merge, WRAPPER_ENV } from '../ci-verify.ts';
import { affectedSet, parseImpact, scorePr, verdict, type PrScore } from '../g14.ts';
import { clippedHint, fenceFor, markdown, packageFolders, readVerify, slowest } from '../monoflow-report.ts';
import { MONOFLOW_COMMIT, WRAPPER, WRAPPER_SCRIPT, strays, writeWrapper } from '../setup-monoflow.ts';
import { dependencyOrder, plan } from '../test.ts';

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
  const root = dir({ 'package.json': JSON.stringify({ workspaces: [] }), '.agents/bin/monoflow': '' });
  const steps = plan(root, { monoflow: true });
  assert.deepEqual(steps.map((s) => s.args.join(' ')), ['--test --test-reporter=dot tools/**/*.test.ts', 'verify --all --test']);
  assert.deepEqual(plan(root, { monoflow: false }).length, 1);
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
