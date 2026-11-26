import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test, { after } from 'node:test';
import { BUDGETS, FIRST_SCREEN_BUDGETS, LAZY_CHUNK_BUDGETS, SERVER_BUDGETS, initialFiles, measure, nearBudget, overBudget, summary, type Measured } from '../budgets.ts';
import { beforeMark } from '../first-screen.ts';

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
  assert.match(over[0] ?? '', /^initial JS [\d.]+ KB br > 0\.0 KB \(\+\d+ B\)$/);
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
  assert.match(miss, /\| initial JS \|.*❌ over by [\d.]+ KB \(\+\d+ B\) \|/);
});

test('budgets: the CLI exits 1 on a miss, 0 within budget, and appends the summary to --summary', () => {
  const run = (dir: string): { code: number; out: string } => {
    const file = join(dir, 'summary.md');
    try {
      execFileSync(process.execPath, [join(import.meta.dirname, '..', 'budgets.ts'), `--dist=${dir}`, `--summary=${file}`, '--entry-only'], { stdio: 'pipe', env: { ...process.env, GITHUB_ACTIONS: '', GITHUB_STEP_SUMMARY: '' } });
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

// ADR-0105: the first screen is what each look loaded (tools/first-screen.ts), not index.html's tags; the worse look is
// gated, each look's numbers are listed, and what either loaded first isn't counted as lazy
test('budgets: with the first screen measured, the worse look is the initial load and each look has its row', () => {
  const root = dist({ entry: 1000, css: 500 });
  writeFileSync(join(root, 'assets/home-f.js'), randomBytes(3000));
  writeFileSync(join(root, 'assets/phone-g.css'), randomBytes(800));
  const m = measure(root, {
    desktop: { js: ['/assets/index-a.js', '/assets/home-f.js'], css: ['/assets/index-c.css'] },
    phone: { js: ['/assets/index-a.js'], css: ['/assets/index-c.css', '/assets/phone-g.css'] },
  });
  assert.deepEqual(m.initialJs.files, ['/assets/index-a.js', '/assets/home-f.js']);
  assert.deepEqual(m.initialCss.files, ['/assets/index-c.css', '/assets/phone-g.css']);
  assert.ok(m.looks?.desktop && m.looks.phone);
  assert.equal(m.lazy.some((l) => l.file === 'assets/home-f.js'), false, 'loaded first: not lazy');
  const sum = summary(m);
  assert.match(sum, /\| first screen JS \(desktop\) \|/);
  assert.match(sum, /\| first screen CSS \(phone\) \|/);
});

test('budgets: the first screen is the JS and CSS fetched before the mark, each once', () => {
  const got = beforeMark(
    [
      { path: '/assets/index-a.js', start: 1 },
      { path: '/assets/index-c.css', start: 2 },
      { path: '/api/events', start: 3 },
      { path: '/assets/index-a.js', start: 4 },
      { path: '/assets/later-z.js', start: 50 },
    ],
    10,
  );
  assert.deepEqual(got, { js: ['/assets/index-a.js'], css: ['/assets/index-c.css'] });
});

test('budgets: no build is a failure that says how to make one', () => {
  const empty = mkdtempSync(join(tmpdir(), 'wisp-budgets-'));
  roots.push(empty);
  assert.throws(() => execFileSync(process.execPath, [join(import.meta.dirname, '..', 'budgets.ts'), `--dist=${empty}`], { stdio: 'pipe' }), (err: { status: number; stderr: Buffer }) => err.status === 1 && /no build at/.test(String(err.stderr)));
});

test('budgets: spikes/server-budgets gates on SERVER_BUDGETS, the one list of the numbers', () => {
  const src = readFileSync(join(import.meta.dirname, '..', '..', 'spikes', 'server-budgets', 'src', 'main.rs'), 'utf8');
  const constant = (name: string): number => Number(new RegExp(`const ${name}: f64 = ([0-9._]+);`).exec(src)?.[1]?.replaceAll('_', ''));
  assert.deepEqual(
    {
      coldStartMs: constant('COLD_START_MS'),
      idleRssMb: constant('IDLE_RSS_MB'),
      listMs: constant('LIST_MS'),
      patchP99Ms: constant('PATCH_P99_MS'),
      appendPerS: constant('APPEND_PER_S'),
      writeAmplification: constant('WRITE_AMPLIFICATION'),
    },
    { ...SERVER_BUDGETS },
  );
});

// ADR-0105 §4: the first screen is gated per look at a ratchet (today's numbers plus 2 KB, 2026-11-19), which only goes
// down toward ADR-0011's 120 / 30; index.html's own files stay gated at 120 / 30 as before
test('budgets: the ratchet is pinned here (lowering it is a deliberate edit, raising it needs an ADR) and never under the target', () => {
  const KB = 1024;
  assert.deepEqual(FIRST_SCREEN_BUDGETS, { desktop: { jsBr: 173 * KB, cssBr: 30 * KB }, phone: { jsBr: 167 * KB, cssBr: 40.5 * KB } });
  for (const b of Object.values(FIRST_SCREEN_BUDGETS)) {
    assert.ok(b.jsBr >= BUDGETS.initialJsBr && b.cssBr >= BUDGETS.initialCssBr, 'a ratchet below the target is the target: lower BUDGETS instead');
  }
});

test('budgets: each look is gated at its own ratchet and the entry at the target; a miss names the look', () => {
  const root = dist({ entry: 1000, css: 500 });
  writeFileSync(join(root, 'assets/home-f.js'), randomBytes(3000));
  writeFileSync(join(root, 'assets/phone-g.css'), randomBytes(800));
  const m = measure(root, {
    desktop: { js: ['/assets/index-a.js', '/assets/home-f.js'], css: ['/assets/index-c.css'] },
    phone: { js: ['/assets/index-a.js'], css: ['/assets/index-c.css', '/assets/phone-g.css'] },
  });
  assert.deepEqual(overBudget(m), []);
  // the desktop's JS over its ratchet: only that is named
  const tight = { desktop: { jsBr: 3500, cssBr: 64 * 1024 }, phone: { jsBr: 64 * 1024, cssBr: 64 * 1024 } };
  const over = overBudget(m, BUDGETS, tight);
  assert.equal(over.length, 1);
  assert.match(over[0] ?? '', /^first screen JS \(desktop\) [\d.]+ KB br > 3\.4 KB \(\+\d+ B\)$/);
  // the entry (index.html's tags) over the target fails even when every look is within its ratchet
  assert.match(overBudget(m, { initialJsBr: 10, initialCssBr: BUDGETS.initialCssBr }).join('\n'), /^initial JS [\d.]+ KB br > 0\.0 KB \(\+\d+ B\)$/m);
  // the summary: each look's row against its ratchet, the entry's against the target
  const sum = summary(m, BUDGETS, tight);
  assert.match(sum, /\| first screen JS \(desktop\) \|.*\| 3\.4 KB \|.*❌ over by/);
  assert.match(sum, /\| first screen CSS \(phone\) \|.*\| 64\.0 KB \|.*✅ \|/);
  assert.match(sum, /\| initial JS \| [\d.]+ KB \| 120\.0 KB \|.*✅ \|/);
});

test('budgets: a lazy chunk over its own ceiling is a miss that names it; mermaid is never on the first screen', () => {
  const root = dist();
  writeFileSync(join(root, 'assets/mermaid.core-h1.js'), randomBytes(3000));
  writeFileSync(join(root, 'assets/elk-i2.js'), randomBytes(2000));
  const lazyOnly = measure(root, { desktop: { js: ['/assets/index-a.js'], css: ['/assets/index-c.css'] } });
  assert.ok(lazyOnly.lazy.some((l) => l.file === 'assets/mermaid.core-h1.js'));
  assert.deepEqual(overBudget(lazyOnly), []);
  const tight = overBudget(lazyOnly, BUDGETS, FIRST_SCREEN_BUDGETS, [{ name: 'mermaid', pattern: /^assets\/mermaid\.core-[\w-]+\.js$/, brMax: 1024 }], 1024);
  assert.ok(tight.some((o) => /^lazy chunk mermaid mermaid\.core-h1\.js [\d.]+ KB br > 1\.0 KB \(\+\d+ B\)$/.test(o)), tight.join('\n'));
  assert.ok(tight.some((o) => /^lazy chunk elk-i2\.js /.test(o)), tight.join('\n'));
  // the real build's mermaid entry is named, and a look that loads it first fails
  const mermaid = LAZY_CHUNK_BUDGETS.find((b) => b.name === 'mermaid');
  assert.ok(mermaid?.neverFirst && mermaid.pattern.test('assets/mermaid.core-Cg3TSjcw.js'));
  const first = measure(root, { phone: { js: ['/assets/index-a.js', '/assets/mermaid.core-h1.js'], css: ['/assets/index-c.css'] } });
  assert.deepEqual(overBudget(first), ['mermaid chunk mermaid.core-h1.js is on the first screen: it loads lazily only']);
});

test('budgets: a miss says its overage in bytes (0.1 KB hid it); a budget met with under 512 B to spare is a warning, not a miss', () => {
  const KB = 1024;
  const part = (br: number): { files: string[]; raw: number; br: number } => ({ files: ['/assets/x.js'], raw: br * 3, br });
  const at = (phoneJs: number): Measured => ({
    entry: { js: part(100 * KB), css: part(20 * KB) },
    initialJs: part(phoneJs),
    initialCss: part(20 * KB),
    looks: { desktop: { js: part(150 * KB), css: part(20 * KB) }, phone: { js: part(phoneJs), css: part(20 * KB) } },
    lazy: [],
    lazyCss: [],
  });
  const budget = FIRST_SCREEN_BUDGETS.phone.jsBr;
  // 212 B over: the bytes say by how much (at 0.1 KB a miss under 52 B read "167.0 KB br > 167.0 KB")
  const over = at(budget + 212);
  assert.deepEqual(overBudget(over), ['first screen JS (phone) 167.2 KB br > 167.0 KB (+212 B)']);
  assert.match(summary(over), /\| first screen JS \(phone\) \|.*❌ over by 0\.2 KB \(\+212 B\) \|/);
  // 100 B under: no miss, a warning naming what's left
  const near = at(budget - 100);
  assert.deepEqual(overBudget(near), []);
  assert.deepEqual(nearBudget(near), ['first screen JS (phone) 166.9 KB br is 100 B under its 167.0 KB budget']);
  assert.match(summary(near), /\| first screen JS \(phone\) \|.*⚠️ 100 B under \|/);
  // well under: neither
  assert.deepEqual(nearBudget(at(budget - 4 * KB)), []);
  // a lazy chunk near its ceiling too
  assert.deepEqual(nearBudget({ ...at(budget - 4 * KB), lazy: [{ file: 'assets/elk-a.js', br: 1024 - 12 }] }, BUDGETS, FIRST_SCREEN_BUDGETS, [], 1024), ['lazy chunk elk-a.js 1.0 KB br is 12 B under its 1.0 KB ceiling']);
});

test('budgets: a miss smaller than 0.1 KB still shows, in bytes', () => {
  const part = (br: number): { files: string[]; raw: number; br: number } => ({ files: ['/assets/x.js'], raw: br, br });
  const m: Measured = { entry: { js: part(BUDGETS.initialJsBr + 30), css: part(1) }, initialJs: part(1), initialCss: part(1), lazy: [], lazyCss: [] };
  assert.deepEqual(overBudget(m), ['initial JS 120.0 KB br > 120.0 KB (+30 B)']);
});
