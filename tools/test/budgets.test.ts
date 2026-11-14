import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test, { after } from 'node:test';
import { BUDGETS, initialFiles, measure, overBudget, summary } from '../budgets.ts';

const roots: string[] = [];
after(() => {
  for (const r of roots) rmSync(r, { recursive: true, force: true });
});

/** A built dist/: index.html naming the entry, one preload and one stylesheet, plus a lazy chunk and a lazy sheet. */
function dist(sizes: { entry?: number; css?: number } = {}): string {
  const root = mkdtempSync(join(tmpdir(), 'wisp-budgets-'));
  roots.push(root);
  mkdirSync(join(root, 'assets'));
  // random bytes don't compress: the brotli size tracks the length
  const noise = (n: number): Buffer => randomBytes(n);
  writeFileSync(
    join(root, 'index.html'),
    '<script type="module" crossorigin src="/assets/index-a.js"></script><link rel="modulepreload" crossorigin href="/assets/solid-b.js"><link rel="stylesheet" crossorigin href="/assets/index-c.css">',
  );
  writeFileSync(join(root, 'assets/index-a.js'), noise(sizes.entry ?? 1000));
  writeFileSync(join(root, 'assets/solid-b.js'), 'export const s=1;');
  writeFileSync(join(root, 'assets/index-c.css'), noise(sizes.css ?? 500));
  writeFileSync(join(root, 'assets/TaskView-d.js'), 'export default 1;');
  writeFileSync(join(root, 'assets/TaskView-e.css'), '[data-kit=x]{color:red}');
  return root;
}

test('budgets: the initial files are the module script, every modulepreload and every stylesheet index.html names', () => {
  const html = readFileSync(join(dist(), 'index.html'), 'utf8');
  assert.deepEqual(initialFiles(html), { js: ['/assets/index-a.js', '/assets/solid-b.js'], css: ['/assets/index-c.css'] });
});

test('budgets: lazy JS and lazy CSS are listed apart from the initial load, never counted in it', () => {
  const m = measure(dist());
  assert.deepEqual(m.initialJs.files, ['/assets/index-a.js', '/assets/solid-b.js']);
  assert.deepEqual(
    m.lazy.map((l) => l.file),
    ['assets/TaskView-d.js'],
  );
  assert.deepEqual(
    m.lazyCss.map((l) => l.file),
    ['assets/TaskView-e.css'],
  );
  assert.ok(m.initialJs.br > 0 && m.initialJs.br <= m.initialJs.raw + 16);
});

test('budgets: a build within both budgets passes; one over either names that budget with its numbers', () => {
  const small = measure(dist());
  assert.deepEqual(overBudget(small), []);
  const over = overBudget(small, { initialJsBr: 10, initialCssBr: 10 });
  assert.equal(over.length, 2);
  assert.match(over[0] ?? '', /^initial JS [\d.]+ KB br > 0\.0 KB$/);
  assert.match(over[1] ?? '', /^initial CSS /);
});

test('budgets: the job summary has a row per budget with the miss, and the lazy chunks', () => {
  const m = measure(dist());
  const ok = summary(m);
  assert.match(ok, /\| initial JS \| [\d.]+ KB \| 120\.0 KB \|.*✅ \|/);
  assert.match(ok, /\| initial CSS \| [\d.]+ KB \| 30\.0 KB \|.*✅ \|/);
  assert.match(ok, /Lazy JS: 1 chunks, largest `TaskView-d\.js`/);
  assert.match(ok, /Lazy CSS: 1 sheets, largest `TaskView-e\.css`/);
  const miss = summary(m, { initialJsBr: 10, initialCssBr: BUDGETS.initialCssBr });
  assert.match(miss, /\| initial JS \|.*❌ over by [\d.]+ KB \|/);
});

test('budgets: the CLI exits 1 on a miss, 0 within budget, and appends the summary to --summary', () => {
  const run = (dir: string): { code: number; out: string } => {
    const file = join(dir, 'summary.md');
    try {
      execFileSync(process.execPath, [join(import.meta.dirname, '..', 'budgets.ts'), `--dist=${dir}`, `--summary=${file}`], { stdio: 'pipe', env: { ...process.env, GITHUB_ACTIONS: '', GITHUB_STEP_SUMMARY: '' } });
      return { code: 0, out: readFileSync(file, 'utf8') };
    } catch (err) {
      return { code: (err as { status: number }).status, out: readFileSync(file, 'utf8') };
    }
  };
  const fine = run(dist());
  assert.equal(fine.code, 0);
  assert.match(fine.out, /### Bundle budgets/);
  const big = run(dist({ entry: BUDGETS.initialJsBr + 4096 }));
  assert.equal(big.code, 1);
  assert.match(big.out, /initial JS .*❌ over by/);
});

test('budgets: no build is a failure that says how to make one', () => {
  const empty = mkdtempSync(join(tmpdir(), 'wisp-budgets-'));
  roots.push(empty);
  assert.throws(() => execFileSync(process.execPath, [join(import.meta.dirname, '..', 'budgets.ts'), `--dist=${empty}`], { stdio: 'pipe' }), (err: { status: number; stderr: Buffer }) => err.status === 1 && /no build at/.test(String(err.stderr)));
});
