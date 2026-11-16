// `npx vitest run <file>` from the repo root: every workspace with a vitest.config.ts is a project, so a file runs
// under its own package's config (tools/vitest.projects.ts). Named vite.config.ts, which vitest also loads, not
// vitest.config.*: monoflow reads a root vitest.config.* as a repo-wide test setup (see tools/vitest.ci.config.ts).
// Nothing runs vite from the root; the apps have their own vite configs.
import { defineConfig } from 'vitest/config';
import { projectConfigs, workspaceVitestDirs } from './tools/vitest.projects.ts';

export default defineConfig({ test: { projects: projectConfigs(workspaceVitestDirs()) } });
