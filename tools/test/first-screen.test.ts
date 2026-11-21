// tools/first-screen.ts's cleanup (CI runs 37dca4f8, 83324057, bb030486: `npm run budgets` hung until the 60-minute job
// limit after `ENOTEMPTY ... rmdir '/tmp/wisp-first-screen-*/Default'`). A fake browser stands in for Chrome: its main
// process prints the DevTools line (or doesn't, or exits), and a helper in its process group keeps writing into the
// profile's Default/ for 600 ms after it's told to stop, as Chrome's helpers do while they shut down.
import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test, { after } from 'node:test';
import { firstScreen, startBrowser, type Served } from '../first-screen.ts';

const unix = process.platform !== 'win32';
const root = mkdtempSync(join(tmpdir(), 'wisp-fake-browser-'));
after(() => rmSync(root, { recursive: true, force: true }));

const FAKE = join(root, 'fake-browser.mjs');
writeFileSync(
  FAKE,
  `import { spawn } from 'node:child_process';
import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
const [mode, ...rest] = process.argv.slice(2);
const dir = rest.find((a) => a.startsWith('--user-data-dir=')).slice('--user-data-dir='.length);
appendFileSync(process.env.FAKE_PIDS, JSON.stringify({ pid: process.pid, dir }) + '\\n');
if (mode === 'helper') {
  mkdirSync(dir + '/Default', { recursive: true });
  process.stdout.write('up\\n');
  let n = 0;
  setInterval(() => writeFileSync(dir + '/Default/f' + n++, 'x'), 5);
  let ending = false;
  const end = () => { if (!ending) { ending = true; setTimeout(() => process.exit(0), 600); } };
  process.on('SIGTERM', end);
  const parent = process.ppid;
  setInterval(() => { if (process.ppid !== parent) end(); }, 10);
} else if (mode === 'exits') {
  process.exit(3);
} else {
  // up once its helper is: 'up' then says it's listening (on a port nothing answers), 'silent' never does
  const h = spawn(process.execPath, [process.argv[1], 'helper', ...rest], { stdio: ['ignore', 'pipe', 'ignore'] });
  h.stdout.once('data', () => {
    if (mode === 'up') console.error('DevTools listening on ws://127.0.0.1:1/devtools/browser/fake');
  });
  setInterval(() => {}, 1000);
}
`,
);

let n = 0;
/** A fake browser in `mode`, and what its processes were. */
function fake(mode: 'up' | 'silent' | 'exits'): { browser: { bin: string; prefix: string[] }; procs: () => { pid: number; dir: string }[] } {
  const pids = join(root, `pids-${n++}`);
  writeFileSync(pids, '');
  process.env['FAKE_PIDS'] = pids;
  const procs = (): { pid: number; dir: string }[] =>
    readFileSync(pids, 'utf8')
      .split('\n')
      .filter(Boolean)
      .map((l) => JSON.parse(l) as { pid: number; dir: string });
  return { browser: { bin: process.execPath, prefix: [FAKE, mode] }, procs };
}

const alive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (err) {
    return (err as NodeJS.ErrnoException).code === 'EPERM';
  }
};

const until = async (what: string, f: () => boolean): Promise<void> => {
  const by = Date.now() + 5_000;
  while (!f()) {
    assert.ok(Date.now() < by, what);
    await new Promise((ok) => setTimeout(ok, 10));
  }
};

/** A server in vite preview's place: whether every browser process was gone when it was closed. */
function server(procs: () => { pid: number }[]): { serve: () => Promise<Served>; closed: boolean[] } {
  const closed: boolean[] = [];
  return { closed, serve: async () => ({ url: 'http://127.0.0.1:1/', close: async () => void closed.push(procs().every((p) => !alive(p.pid))) }) };
}

test('first-screen: stop ends every process of the browser, and only then removes its profile', { skip: !unix && 'process groups: Unix', timeout: 30_000 }, async () => {
  const f = fake('up');
  const b = startBrowser({ ...f.browser });
  try {
    await b.ready;
    await until('the helper started', () => f.procs().length === 2 && existsSync(join(b.dir, 'Default')));
  } finally {
    await b.stop();
  }
  for (const p of f.procs()) assert.equal(alive(p.pid), false, `pid ${p.pid} is gone once stop resolves`);
  assert.equal(existsSync(b.dir), false, 'the profile is removed');
  await b.stop();
});

test('first-screen: a measure that fails (DevTools unreachable) rejects at once, saying why, with everything stopped', { skip: !unix && 'process groups: Unix', timeout: 30_000 }, async () => {
  const f = fake('up');
  const s = server(f.procs);
  const t0 = Date.now();
  await assert.rejects(firstScreen('/nowhere', root, { browser: f.browser, serve: s.serve, deadlineMs: 20_000 }), /DevTools socket/);
  assert.ok(Date.now() - t0 < 10_000, `settled in ${Date.now() - t0} ms`);
  assert.deepEqual(s.closed, [true], 'the server closed once, after the browser was gone');
  const procs = f.procs();
  assert.equal(procs.length, 2, 'the main process and its helper');
  for (const p of procs) assert.equal(alive(p.pid), false, `pid ${p.pid} is gone`);
  assert.equal(existsSync(procs[0]!.dir), false, 'no profile left');
});

test('first-screen: past the deadline the measure fails with its own message, the browser and server stopped', { skip: !unix && 'process groups: Unix', timeout: 30_000 }, async () => {
  const f = fake('silent');
  const s = server(f.procs);
  const t0 = Date.now();
  // the helper is up well within it (two Node starts), so both are there to be stopped
  await assert.rejects(firstScreen('/nowhere', root, { browser: f.browser, serve: s.serve, deadlineMs: 3_000 }), /not measured in 3 s/);
  assert.ok(Date.now() - t0 < 10_000, `settled in ${Date.now() - t0} ms`);
  assert.deepEqual(s.closed, [true]);
  const procs = f.procs();
  assert.equal(procs.length, 2, 'the main process and its helper');
  for (const p of procs) assert.equal(alive(p.pid), false, `pid ${p.pid} is gone`);
  assert.equal(existsSync(procs[0]!.dir), false, 'no profile left');
});

test('first-screen: a browser that exits at once is a failure that says so, the server still closed', { timeout: 30_000 }, async () => {
  const f = fake('exits');
  const s = server(f.procs);
  await assert.rejects(firstScreen('/nowhere', root, { browser: f.browser, serve: s.serve, deadlineMs: 20_000 }), /Chrome exited \(code 3\)/);
  assert.deepEqual(s.closed, [true]);
  assert.equal(existsSync(f.procs()[0]!.dir), false, 'no profile left');
});
