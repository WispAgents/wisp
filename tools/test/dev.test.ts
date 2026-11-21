// tools/dev/plan.ts: npm run dev's arguments, port choice, homes and the env each child gets.

import assert from 'node:assert/strict';
import test from 'node:test';
import {
  apiUrl, builtBinary, choosePort, DEFAULT_PORT, DESKTOP_DEV_IDENTIFIER, desktopEnv, DevArgError, isRustChange, parseArgs,
  portCandidates, resolveHome, serverEnv, viteEnv, webIndex,
} from '../dev/plan.ts';

test('the_defaults_are_port_4800_vite_5180_the_dev_home_and_watching', () => {
  assert.deepEqual(parseArgs([]), { desktop: false, port: 4800, vitePort: 5180, home: undefined, watch: true, help: false });
});

test('flags_and_env_set_the_options', () => {
  const a = parseArgs(['--desktop', '--port', '4900', '--vite-port=5300', '--home', '/tmp/h', '--no-watch']);
  assert.deepEqual(a, { desktop: true, port: 4900, vitePort: 5300, home: '/tmp/h', watch: false, help: false });
  assert.equal(parseArgs([], { WISP_DEV_PORT: '4810', WISP_DEV_HOME: '/tmp/e' }).port, 4810);
  assert.equal(parseArgs(['--port', '4820'], { WISP_DEV_PORT: '4810' }).port, 4820, 'a flag beats the env');
  assert.equal(parseArgs(['-h']).help, true);
});

test('bad_arguments_and_an_installed_wisps_port_are_refused', () => {
  assert.throws(() => parseArgs(['--port', '4477']), DevArgError);
  assert.throws(() => parseArgs(['--vite-port', '4791']), DevArgError);
  assert.throws(() => parseArgs([], { WISP_DEV_PORT: '4477' }), DevArgError);
  assert.throws(() => parseArgs(['--port', 'x']), DevArgError);
  assert.throws(() => parseArgs(['--port']), DevArgError);
  assert.throws(() => parseArgs(['--home']), DevArgError);
  assert.throws(() => parseArgs(['--what']), DevArgError);
});

test('port_candidates_skip_4477_and_4791', () => {
  const c = portCandidates(4470, 400);
  assert.equal(c[0], 4470);
  assert.ok(!c.includes(4477) && !c.includes(4791));
  assert.equal(portCandidates(DEFAULT_PORT, 3).join(), '4800,4801,4802');
});

test('a_busy_port_falls_back_to_the_next_free_one_and_never_reuses_a_taken_one', async () => {
  const busy = new Set([4800, 4801]);
  assert.equal(await choosePort(4800, async (p) => !busy.has(p)), 4802);
  assert.equal(await choosePort(4802, async (p) => !busy.has(p), [4802]), 4803);
  assert.equal(await choosePort(4476, async (p) => p >= 4477), 4478, 'never 4477 even when free');
  await assert.rejects(choosePort(4800, async () => false), DevArgError);
});

test('the_home_is_the_dev_home_by_default_and_never_an_installed_wisps', () => {
  for (const os of ['linux', 'darwin']) {
    const home = (h: string | undefined): string => resolveHome(h, '/Users/u', '/repo', os);
    assert.equal(home(undefined), '/Users/u/.wisp-dev');
    assert.equal(home('~/x'), '/Users/u/x');
    assert.equal(home('rel'), '/repo/rel');
    for (const h of ['/Users/u/.wisp', '~/.wisp', '/Users/u/.wisp/sub', '/Users/u/.wisp-v2-preview', '/Users/u/.wisp/../.wisp']) {
      assert.throws(() => home(h), DevArgError, `${os} ${h}`);
    }
    assert.equal(home('/Users/u/.wisp-devx'), '/Users/u/.wisp-devx');
    assert.equal(home('~'), '/Users/u', '~ alone is the user home');
    assert.throws(() => home('/Users/u/.wisp/..x'), DevArgError, 'a folder named ..x is still inside');
  }
  assert.throws(() => resolveHome('~/.WISP', '/Users/u', '/repo', 'darwin'), DevArgError, 'macOS folds case');
  assert.throws(() => resolveHome('~/.Wisp-V2-Preview/x', '/Users/u', '/repo', 'win32'), DevArgError);
  assert.equal(resolveHome('~/.WISP', '/Users/u', '/repo', 'linux'), '/Users/u/.WISP', 'Linux does not');
});

test('the_dev_home_on_windows_is_a_windows_path_and_refuses_the_installed_home_however_it_is_written', () => {
  const home = (h: string | undefined): string => resolveHome(h, 'C:\\Users\\u', 'D:\\repo', 'win32');
  assert.equal(home(undefined), 'C:\\Users\\u\\.wisp-dev');
  assert.equal(home('~/x'), 'C:\\Users\\u\\x');
  assert.equal(home('~\\x'), 'C:\\Users\\u\\x', 'a Windows ~\\ expands too');
  assert.equal(home('rel'), 'D:\\repo\\rel');
  assert.equal(home('~'), 'C:\\Users\\u');
  assert.equal(home('E:\\.wisp'), 'E:\\.wisp', 'another drive is not the installed home');
  for (const h of ['C:\\Users\\u\\.wisp', 'c:/users/U/.WISP', '~\\.wisp', '~/.wisp/sub', 'C:\\Users\\u\\.wisp-v2-preview\\x', 'C:\\Users\\u\\.wisp\\..\\.wisp']) {
    assert.throws(() => home(h), DevArgError, h);
  }
});

test('the_server_gets_its_port_home_and_stdin_exit_and_no_desktop_or_web_dir_env', () => {
  const env = serverEnv({ PATH: '/bin', WISP_WEB_DIR: '/w', WISP_DESKTOP: 'tauri', WISP_DESKTOP_DATA: '/d', WISP_PORT: '4477', WISP_HOME: '/Users/u/.wisp' }, 4801, '/Users/u/.wisp-dev');
  assert.deepEqual(env, { PATH: '/bin', WISP_PORT: '4801', WISP_HOME: '/Users/u/.wisp-dev', WISP_EXIT_WITH_STDIN: '1' });
});

test('vite_gets_its_port_the_real_api_and_no_mock', () => {
  assert.deepEqual(viteEnv({ PATH: '/bin' }, 5181, 4801), { PATH: '/bin', PORT: '5181', WISP_MOCK: '0', WISP_DEV_API: apiUrl(4801) });
  assert.equal(apiUrl(4801), 'http://127.0.0.1:4801');
});

test('the_desktop_attaches_to_the_dev_port_loads_vite_and_uses_the_dev_identifier', () => {
  const overlay = { identifier: DESKTOP_DEV_IDENTIFIER, productName: 'wisp dev' };
  const env = desktopEnv({ PATH: '/bin', WISP_PORT: '4477', WISP_BIN: '/old/wisp' }, 4801, 5181, '/h', overlay, '/tmp/t', '/t/debug/wisp');
  assert.equal(env['WISP_BIN'], '/t/debug/wisp', 'never a stale or installed server');
  assert.equal(env['WISP_PORT'], '4801');
  assert.equal(env['WISP_HOME'], '/h');
  assert.equal(env['WISP_DEV_URL'], 'http://localhost:5181');
  assert.equal(env['CARGO_TARGET_DIR'], '/tmp/t');
  assert.deepEqual(JSON.parse(env['TAURI_CONFIG'] ?? ''), overlay);
  assert.equal(desktopEnv({ CARGO_TARGET_DIR: '/shared' }, 4801, 5181, '/h', overlay, '/tmp/t', '/b')['CARGO_TARGET_DIR'], '/tmp/t', 'never the server build\'s');
  assert.throws(() => desktopEnv({}, 4801, 5181, '/h', { identifier: 'dev.wisp.app' }, '/tmp/t', '/b'), DevArgError);
});

test('rust_sources_under_crates_and_plugin_hosts_rebuild_and_nothing_else_does', () => {
  for (const p of ['crates/wisp/src/serve.rs', 'crates/wisp-kernel/capabilities.json', 'plugins/git/host/src/lib.rs', 'plugins/git/host/wisp-plugin.json', 'Cargo.toml', 'Cargo.lock']) {
    assert.ok(isRustChange(p), p);
  }
  for (const p of ['plugins/git/ui/src/a.tsx', 'packages/ui/src/a.ts', 'crates/wisp/target/debug/x.rs', 'plugins/git/host/node_modules/a/Cargo.toml', 'crates/wisp/README.md', 'package.json']) {
    assert.ok(!isRustChange(p), p);
  }
});

test('the_built_binary_comes_from_cargos_json_lines', () => {
  const lines = [
    'Compiling …',
    JSON.stringify({ reason: 'compiler-artifact', target: { name: 'wisp_kernel', kind: ['lib'] }, executable: null }),
    JSON.stringify({ reason: 'compiler-artifact', target: { name: 'wisp', kind: ['bin'] }, executable: '/t/debug/wisp' }),
    '{not json',
    JSON.stringify({ reason: 'build-finished', success: true }),
  ];
  assert.equal(builtBinary(lines), '/t/debug/wisp');
  assert.equal(builtBinary(lines.slice(0, 2)), undefined);
});

test('the_web_build_dev_checks_is_the_one_a_debug_wisp_serve_falls_back_to', async () => {
  const { join } = await import('node:path');
  const { readFile } = await import('node:fs/promises');
  const root = join(import.meta.dirname, '..', '..');
  assert.equal(webIndex(root), join(root, 'apps', 'web', 'dist', 'index.html'));
  // serve.rs's web_dir(): CARGO_MANIFEST_DIR (crates/wisp)/../../apps/web/dist, holding an index.html
  const serve = await readFile(join(root, 'crates', 'wisp', 'src', 'serve.rs'), 'utf8');
  assert.match(serve, /join\("\.\.\/\.\.\/apps\/web\/dist"\)/);
  assert.match(serve, /join\("index\.html"\)/);
});
