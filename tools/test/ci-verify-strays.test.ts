// tools/ci-verify.ts ends the background verifies its monoflow calls leave (cold start over budget, PR CI run
// 44807038322: a detached `monoflow verify` and its 4 cargo builds ran through the server budgets step).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { SPEC_DIR, stopStrayRuns, strayRuns } from '../ci-strays.ts';

const alive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

test('a_background_run_is_found_by_its_pid_file_written_since_the_start_and_only_those', () => {
  const since = 1_800_000_000_000;
  const files = [
    { name: 'a1b2.pid', mtimeMs: since + 5, text: '4242\n' },
    { name: 'old.pid', mtimeMs: since - 60_000, text: '777' }, // another checkout's, or an earlier day's
    { name: 'a1b2.out', mtimeMs: since + 9, text: 'PASS' },
    { name: 'a1b2.tmp', mtimeMs: since + 9, text: '' },
    { name: 'bad.pid', mtimeMs: since + 9, text: 'not a pid' },
    { name: 'one.pid', mtimeMs: since + 9, text: '1' }, // never init's group
    { name: 'slack.pid', mtimeMs: since - 500, text: '5151' }, // written within the second before: ours
  ];
  assert.deepEqual(strayRuns(files, since), [4242, 5151]);
  assert.deepEqual(strayRuns([], since), []);
});

/** A detached group like monoflow's background run (`sh -c "… verify"`, setsid): its leader and a child of it. */
function detachedGroup(): Promise<{ leader: number; child: number }> {
  return new Promise((done, fail) => {
    const p = spawn('/bin/sh', ['-c', 'sleep 60 & echo $!; wait'], { detached: true, stdio: ['ignore', 'pipe', 'ignore'] });
    p.stdout.setEncoding('utf8').once('data', (d: string) => done({ leader: p.pid ?? 0, child: Number(d.trim()) }));
    p.once('error', fail);
    p.unref();
  });
}

test('every_background_run_the_calls_left_is_ended_with_its_children_and_an_older_one_is_left_alone', { skip: process.platform === 'win32' }, async () => {
  const root = mkdtempSync(join(tmpdir(), 'wisp-ci-strays-'));
  const dir = join(root, SPEC_DIR);
  mkdirSync(dir, { recursive: true });
  const since = Date.now();
  const ours = await detachedGroup();
  const theirs = await detachedGroup();
  try {
    writeFileSync(join(dir, 'k1.pid'), String(ours.leader));
    writeFileSync(join(dir, 'k0.pid'), String(theirs.leader));
    const before = (since - 120_000) / 1000;
    utimesSync(join(dir, 'k0.pid'), before, before);
    assert.ok(alive(ours.leader) && alive(ours.child), 'the group runs');
    const said: string[] = [];
    assert.deepEqual(await stopStrayRuns(root, since, (l) => said.push(l)), [ours.leader]);
    for (let i = 0; i < 50 && (alive(ours.leader) || alive(ours.child)); i++) await new Promise((r) => setTimeout(r, 20));
    assert.ok(!alive(ours.leader), 'the run is gone');
    assert.ok(!alive(ours.child), 'its child (a cargo build) is gone with it');
    assert.match(said.join(''), /stopped 1 background monoflow run \(process group \d+\)/);
    assert.ok(alive(theirs.leader) && alive(theirs.child), 'a run from before this step is not ours');
    // nothing left: nothing to say
    assert.deepEqual(await stopStrayRuns(root, since), []);
  } finally {
    for (const pid of [ours.leader, theirs.leader]) {
      try {
        process.kill(-pid, 'SIGKILL');
      } catch {
        // gone
      }
    }
    rmSync(root, { recursive: true, force: true });
  }
});
