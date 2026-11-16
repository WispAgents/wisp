// The vitest projects of this repo: one per npm workspace folder that has its own vitest.config.ts. Shared by CI's one
// vitest run (tools/vitest.ci.config.ts, the folders it is handed) and the root config (vite.config.ts, every
// workspace), so `npx vitest run <file>` from the repo root runs that file under its package's config (its plugins,
// environment, setup and solid's browser build) instead of no config at all (solid's server build: effects never run).
import { existsSync, globSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

export const ROOT = join(import.meta.dirname, '..');

/** Each folder's vitest.config.ts, absolute, with forward slashes: vitest reads each entry as a glob, where `\` escapes (Windows paths). */
export function projectConfigs(dirs: readonly string[]): string[] {
  return dirs.map((d) => join(ROOT, d, 'vitest.config.ts').replaceAll('\\', '/'));
}

/** Every workspace folder (the root package.json's `workspaces` globs) that has a vitest.config.ts, relative to the root. */
export function workspaceVitestDirs(): string[] {
  const pj: unknown = JSON.parse(readFileSync(join(ROOT, 'package.json'), 'utf8'));
  const globs = typeof pj === 'object' && pj !== null && 'workspaces' in pj && Array.isArray(pj.workspaces) ? pj.workspaces.filter((w): w is string => typeof w === 'string') : [];
  return globSync(globs, { cwd: ROOT })
    .map((d) => d.replaceAll('\\', '/'))
    .filter((d) => existsSync(join(ROOT, d, 'vitest.config.ts')))
    .sort();
}
