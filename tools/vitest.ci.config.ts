// CI's one vitest run over every npm workspace whose `test` script is plain vitest (tools/ci-verify.ts, ADR-0072).
// Each folder in $WISP_VITEST_PROJECTS (a JSON list, relative to the repo root) is a project with its own
// vitest.config.ts, exactly what `npm run test` in that folder runs; one process and one worker pool instead of a
// vitest start (config bundling, vite server, workers) per package.
// Not named vitest.config.* at the root, so monoflow never takes it for a repo-wide test command.
import { defineConfig } from 'vitest/config';
import { projectConfigs } from './vitest.projects.ts';

const dirs: unknown = JSON.parse(process.env['WISP_VITEST_PROJECTS'] ?? '[]');

export default defineConfig({
  test: {
    // a package whose script has no --passWithNoTests is held to having tests by ci-verify itself
    passWithNoTests: true,
    projects: projectConfigs((Array.isArray(dirs) ? dirs : []).filter((d): d is string => typeof d === 'string')),
  },
});
