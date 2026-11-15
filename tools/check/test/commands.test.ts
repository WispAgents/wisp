import assert from 'node:assert/strict';
import test from 'node:test';
import { commands, segments } from '../rules/commands.ts';
import { messages, pkg, writeTree } from './helpers.ts';

const ROOT = JSON.stringify(
  {
    name: 'fixture',
    private: true,
    type: 'module',
    workspaces: ['packages/*', 'apps/*'],
    scripts: {
      check: 'node tools/check.ts',
      gone: 'node tools/gone.ts && npm run nope',
      web: 'npm run build --workspace @x/web && npm run build -w apps/missing',
      rust: 'cargo run -p wisp -- serve && cargo test -p wisp-nope && cargo run -p wisp -- frobnicate',
      dev: 'node tools/dev.ts',
      env: 'FOO=1 node --import ./tools/reg.ts tools/check.ts | tee out.txt',
      tests: 'node --test tools/a.test.ts tools/*.test.ts tools/b.test.ts',
    },
  },
  null,
  2,
);

const ARGS_RS = [
  'pub const HELP: &str = "usage:',
  '  wisp ls                       list',
  '  wisp diff|validate -f F       plans',
  '  wisp <plugin> <verb> …        any command";',
  'pub const COMMANDS: [&str; 2] = ["serve", "crew"];',
  '',
].join('\n');
const MAIN_RS = 'fn main() {\n    match w {\n        "tasks" if more => x(),\n        Some("plugin" | "plugins") => y(),\n    }\n}\n';

function tree(docs: Record<string, string>): string {
  return writeTree(
    new Map([
      ['package.json', ROOT],
      ['Cargo.toml', '[workspace]\nmembers = ["crates/*"]\n'],
      ['crates/wisp/Cargo.toml', '[package]\nname = "wisp"\n'],
      ['crates/wisp/src/args.rs', ARGS_RS],
      ['crates/wisp/src/main.rs', MAIN_RS],
      ['plugins/runtime/host/wisp-plugin.json', JSON.stringify({ wispPlugin: 1, id: 'wisp.runtime', contributes: { commands: [{ id: 'runtime.usage' }] } })],
      ['tools/check.ts', ''],
      ['tools/reg.ts', ''],
      ['tools/a.test.ts', ''],
      ['tools/setup-monoflow.ts', ''],
      ['qa/layout.mjs', ''],
      ['apps/web/package.json', pkg('@x/web', [], { scripts: { build: 'vite build', gen: './scripts/gen.sh', bad: 'bash scripts/missing.sh && tsx src/main.ts' } })],
      ['apps/web/scripts/gen.sh', ''],
      ['apps/web/src/main.ts', ''],
      ...Object.entries(docs),
    ]),
  );
}

test('commands: segments split on && || ; | and drop comments; quotes are kept as one word', () => {
  assert.deepEqual(
    segments('A=1 npm run x && wisp "fix it" | tee y; z || w # npm run gone').map((s) => s.map((w) => (w.quoted ? `"${w.text}"` : w.text))),
    [['A=1', 'npm', 'run', 'x'], ['wisp', '"fix it"'], ['tee', 'y'], ['z'], ['w']],
  );
});

test('commands: a package.json script naming a missing file, script, workspace or crate fails; what exists passes', async () => {
  const got = await messages(commands, tree({}));
  const want = [
    'apps/web/package.json:1 commands script bad: scripts/missing.sh doesn\'t exist (from apps/web)',
    'package.json:11 commands script gone: tools/gone.ts doesn\'t exist',
    'package.json:11 commands script gone: npm script nope doesn\'t exist in the root package.json',
    'package.json:12 commands script web: npm workspace apps/missing doesn\'t exist',
    'package.json:13 commands script rust: crate wisp-nope is no member of the Cargo workspace',
    'package.json:13 commands script rust: wisp frobnicate is no command of the CLI (crates/wisp/src/args.rs, crates/wisp/src/main.rs): it would launch a task named "frobnicate"',
    // the bug this rule exists for: `npm run dev` pointed at a missing tools/dev.ts for weeks
    'package.json:14 commands script dev: tools/dev.ts doesn\'t exist',
    'package.json:16 commands script tests: tools/b.test.ts doesn\'t exist',
  ];
  assert.deepEqual(got, want.sort(), got.join('\n'));
});

test('commands: documented commands resolve; unknown wisp words, scripts and paths fail; skips need a why', async () => {
  const agents = [
    '# Commands',
    '',
    '```sh',
    'npm run check                # ok',
    'npm run missing              # fails',
    'npm run dev                  # another coder adds it: exempt',
    'npm test                     # no test script: fails',
    'cargo test -p <crate> --test <file>   # placeholder',
    'cargo clippy --workspace --all-targets -- -D warnings',
    'git status && gh pr list     # external',
    'wisp ls && wisp diff -f x && wisp tasks --touching a && wisp plugins && wisp crew && wisp runtime usage',
    'wisp "fix the tests"         # a task, quoted',
    'wisp --mode plan "x"         # a flag first',
    'wisp spell cast x            # fails',
    '.agents/bin/monoflow graph impact <file>   # installed by tools/setup-monoflow.ts',
    'qa/layout.mjs && qa/gone.mjs',
    'cd elsewhere && node tools/nowhere.ts      # after cd: not checked',
    '```',
    '',
    '<!-- cmd-check: skip v1-only, rework pending -->',
    '```sh',
    'wisp usage',
    '```',
    '',
    '```text',
    'wisp nothing   # not a shell block',
    '```',
    '',
    'Run `npm run absent` or `wisp usage`, see `qa/missing.mjs`; `node tools/check.ts` is fine.',
    'Also `wisp blame x` <!-- cmd-check: skip -->',
    'And `wisp share x` <!-- cmd-check: skip on purpose -->',
  ].join('\n');
  const arch = ['```', 'tools/            check', '  wisp/           the binary', 'npm run nada', '```'].join('\n');
  const skill = '1. `node tools/new-plugin.ts <name>` scaffolds, then `.agents/bin/monoflow verify`.\n2. `node tools/missing.ts`\n';
  const root = tree({ 'AGENTS.md': agents, 'ARCHITECTURE.md': arch, '.claude/skills/x/SKILL.md': skill });
  const got = await messages(commands, root);
  const cli = (w: string): string => `wisp ${w} is no command of the CLI (crates/wisp/src/args.rs, crates/wisp/src/main.rs): it would launch a task named "${w}"`;
  const want = [
    'AGENTS.md:5 commands npm script missing doesn\'t exist in the root package.json',
    'AGENTS.md:7 commands npm script test doesn\'t exist in the root package.json',
    `AGENTS.md:14 commands ${cli('spell')}`,
    'AGENTS.md:16 commands qa/gone.mjs doesn\'t exist',
    'AGENTS.md:29 commands npm script absent doesn\'t exist in the root package.json',
    `AGENTS.md:29 commands ${cli('usage')}`,
    'AGENTS.md:29 commands qa/missing.mjs doesn\'t exist',
    'AGENTS.md:30 commands cmd-check: skip needs a why (`<!-- cmd-check: skip <why> -->`)',
    'ARCHITECTURE.md:4 commands npm script nada doesn\'t exist in the root package.json',
    '.claude/skills/x/SKILL.md:1 commands tools/new-plugin.ts doesn\'t exist',
    '.claude/skills/x/SKILL.md:2 commands tools/missing.ts doesn\'t exist',
  ];
  const docs = got.filter((m) => !m.startsWith('package.json') && !m.startsWith('apps/'));
  assert.deepEqual(docs, want.sort(), docs.join('\n'));
});

test('commands: review fixes: cd ends a block, parens split, skips report once, bash x.sh, node --test-* values, wisp v2 prose', async () => {
  const agents = [
    '```sh',
    'npm run check',
    'cd desktop',
    'npm run tauri                # after a cd in this block: not checked',
    '```',
    '',
    '```sh',
    '(npm run nope2) && node --test --test-name-pattern foo --test-reporter spec tools/a.test.ts',
    'bash build.sh && bash tools/setup-monoflow.ts',
    '(cd desktop && npm run tauri)  # no `tauri)` word, and a cd ends the block',
    'npm run tauri',
    '```',
    '',
    '<!-- cmd-check: skip -->',
    '```sh',
    'npm run nope3',
    '```',
    '',
    'wisp v2 is `wisp v2`; `wisp 2.0`, `wisp: error`. Quoted syntax `<!-- cmd-check: skip -->` is no skip: `npm run nope4`.',
  ].join('\n');
  const got = (await messages(commands, tree({ 'AGENTS.md': agents }))).filter((m) => m.startsWith('AGENTS.md'));
  const want = [
    'AGENTS.md:8 commands npm script nope2 doesn\'t exist in the root package.json',
    'AGENTS.md:9 commands build.sh doesn\'t exist',
    'AGENTS.md:14 commands cmd-check: skip needs a why (`<!-- cmd-check: skip <why> -->`)',
    'AGENTS.md:19 commands npm script nope4 doesn\'t exist in the root package.json',
  ];
  assert.deepEqual(got, want.sort(), got.join('\n'));
});

test('commands: a cited path that exists only because it is git-ignored fails like in CI; a tracked file passes', async () => {
  const { execFileSync } = await import('node:child_process');
  const { mkdirSync, writeFileSync } = await import('node:fs');
  const { join } = await import('node:path');
  const root = tree({ 'docs/dev.md': '# Dev\n\n```sh\nnode tools/check.ts\nnode tools/out/report.ts\n```\n', '.gitignore': 'tools/out/\n' });
  mkdirSync(join(root, 'tools/out'), { recursive: true });
  writeFileSync(join(root, 'tools/out/report.ts'), '');
  const git = (...a: string[]): void => void execFileSync('git', a, { cwd: root, stdio: 'ignore' });
  git('init', '-q');
  git('add', '.');
  const got = (await messages(commands, root)).filter((m) => m.startsWith('docs/dev.md'));
  assert.deepEqual(got, ["docs/dev.md:5 commands tools/out/report.ts is git-ignored: it exists here but won't in CI (name a tracked file, or add it to CREATED in tools/check/rules/commands.ts)"], got.join('\n'));
});
