// The background runs tools/ci-verify.ts's monoflow calls leave behind, found and ended before its step does, so
// nothing it started runs through the steps after it (the server budgets step times a cold start).
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

/** Where monoflow keeps its background verifies' pid files (`<key>.pid`, cmd/monoflow/speculate.go specDir). */
export const SPEC_DIR = join('.agents', 'verify');

/**
 * The background verifies a run left: `monoflow verify --package=…` starts one for the change's dependents whenever the
 * checkout has uncommitted or untracked files, MONOFLOW_NO_SPECULATE or not (cmd_verify.go `ensureSpecRun`, at the pin).
 * It runs detached (setsid), so it outlived ci-verify and kept 4 cargo builds going through the server budgets step:
 * cold start 92 → 167 ms on the same runners, the list read 1.2 → 3.2 ms, from 9e15602f on (each job's
 * "Terminate orphan process: monoflow-bin, cargo, rustc"). Each one's pid file holds its process group's leader. Only
 * the files written since `since` (ms since the epoch, a second's slack): another checkout's runs aren't ours. Pure.
 */
export function strayRuns(files: readonly { readonly name: string; readonly mtimeMs: number; readonly text: string }[], since: number): number[] {
  return files
    .filter((f) => f.name.endsWith('.pid') && f.mtimeMs >= since - 1000)
    .map((f) => Number(f.text.trim()))
    .filter((pid) => Number.isInteger(pid) && pid > 1);
}

const alive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

/** Ends the background verifies the calls left (each pid file's process group: SIGTERM, then SIGKILL after 2 s, as
 * monoflow's own `killGroup` signals it) and says so. POSIX only: on Windows a detached run has no group to signal. */
export async function stopStrayRuns(root: string, since: number, say: (line: string) => void = () => {}): Promise<number[]> {
  if (process.platform === 'win32') return [];
  const dir = join(root, SPEC_DIR);
  const names = existsSync(dir) ? readdirSync(dir) : [];
  const files = names.map((name) => {
    try {
      return { name, mtimeMs: statSync(join(dir, name)).mtimeMs, text: name.endsWith('.pid') ? readFileSync(join(dir, name), 'utf8') : '' };
    } catch {
      return { name, mtimeMs: 0, text: '' };
    }
  });
  const pids = strayRuns(files, since).filter(alive);
  if (!pids.length) return [];
  const signal = (sig: NodeJS.Signals): void => {
    for (const pid of pids) {
      try {
        process.kill(-pid, sig);
      } catch {
        // the group is gone already
      }
    }
  };
  signal('SIGTERM');
  for (let i = 0; i < 20 && pids.some(alive); i++) await new Promise((r) => setTimeout(r, 100));
  signal('SIGKILL');
  say(`ci-verify: stopped ${pids.length} background monoflow run${pids.length === 1 ? '' : 's'} (process group ${pids.join(', ')}): nothing outlives this step\n`);
  return pids;
}
