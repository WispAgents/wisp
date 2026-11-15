// tools/doctor/plan.ts: npm run doctor's decisions: versions, the disk threshold, lockfile drift, the toolchain,
// what blocks versus what is advice, and the report.

import assert from 'node:assert/strict';
import test from 'node:test';
import {
  compareVersions, depsCheck, desktopCheck, DISK_WARN_BYTES, diskCheck, exitCode, fakeAgentsCheck, format, hooksCheck, lockDrift,
  monoflowCheck, nodeCheck, parseToolchainToml, satisfies, toolchainChecks, type ToolchainFacts, xcodeCheck,
} from '../doctor/plan.ts';

test('versions_compare_numerically_not_as_strings', () => {
  assert.ok(compareVersions('v22.18.0', '22.9.0') > 0, '18 > 9');
  assert.ok(compareVersions('1.100.0', '1.99.1') > 0);
  assert.equal(compareVersions('v24.1', '24.1.0'), 0, 'missing parts are 0');
  assert.ok(compareVersions('v20.19.5', '22.18') < 0);
  assert.ok(compareVersions('nope', '1.0.0') < 0, 'unparsable sorts first');
});

test('engines_ranges_accept_and_refuse', () => {
  assert.equal(satisfies('v22.18.0', '>=22.18'), true);
  assert.equal(satisfies('v26.12.0', '>=22.18'), true);
  assert.equal(satisfies('v22.17.9', '>=22.18'), false);
  assert.equal(satisfies('v20.0.0', '>=22.18 <25'), false);
  assert.equal(satisfies('v25.0.0', '>=22.18 <25'), false, 'every part must hold');
  assert.equal(satisfies('v20.19.0', '^20.19 || >=22.12'), true, 'any alternative');
  assert.equal(satisfies('v21.0.0', '^20.19 || >=22.12'), false);
  assert.equal(satisfies('v22.3.1', '22'), true, 'a bare major is that major');
  assert.equal(satisfies('v1.0.0', 'lts/*'), true, 'a range it cannot read never blocks');
});

test('engines_ranges_read_spaced_ops_hyphens_tilde_majors_and_partial_bounds_as_semver', () => {
  assert.equal(satisfies('v24.1.0', '>= 22.18'), true, 'a space after the operator');
  assert.equal(satisfies('v22.5.0', '>= 22.18'), false);
  assert.equal(satisfies('v24.1.0', '22 - 24'), true, 'a hyphen range includes the whole upper major');
  assert.equal(satisfies('v25.0.0', '22 - 24'), false);
  assert.equal(satisfies('v22.5.0', '~22'), true, '~ with only a major is that major');
  assert.equal(satisfies('v24.1.0', '<=24'), true, '<=24 means <25');
  assert.equal(satisfies('v22.5.0', '>22'), false, '>22 means >=23');
  assert.equal(satisfies('v23.0.0', '>22'), true);
  assert.equal(satisfies('v22.18.1', '>22.18.0'), true, 'a full version compares exactly');
  assert.equal(satisfies('v0.3.0', '^0.2'), false, '^0.x stays in its minor');
  assert.equal(satisfies('v24.0.0', '24.x'), true);
});

test('an_old_node_blocks_with_the_fix', () => {
  const c = nodeCheck('v20.11.0', '>=22.18');
  assert.equal(c.status, 'fail');
  assert.match(c.fix ?? '', /22\.18/);
  assert.equal(nodeCheck('v24.0.0', '>=22.18').status, 'ok');
  assert.equal(nodeCheck('v18.0.0', undefined).status, 'ok', 'no engines: nothing to check');
});

test('disk_under_20_gb_warns_and_never_blocks', () => {
  assert.equal(diskCheck(DISK_WARN_BYTES - 1, '/t').status, 'warn');
  assert.equal(diskCheck(1, '/t').status, 'warn', 'even nearly full is advice');
  assert.equal(diskCheck(DISK_WARN_BYTES, '/t').status, 'ok', '20 GB exactly is enough');
  assert.equal(diskCheck(undefined, '/t').status, 'info');
  assert.match(diskCheck(5 * 1024 ** 3, '/t').detail, /5\.0 GB free/);
});

test('lock_drift_finds_missing_and_changed_and_skips_platform_optionals', () => {
  const lock = {
    packages: {
      '': { version: '1.0.0' },
      'packages/ui': { version: '2.0.0' },
      'node_modules/@wisp/ui': { link: true },
      'node_modules/solid-js': { version: '1.9.5' },
      'node_modules/vite': { version: '7.0.0' },
      'node_modules/typescript': { version: '5.9.2' },
      'node_modules/@esbuild/linux-x64': { version: '0.25.0', optional: true },
    },
  };
  const installed = { packages: { 'node_modules/@wisp/ui': { link: true }, 'node_modules/solid-js': { version: '1.9.4' }, 'node_modules/typescript': { version: '5.9.2' } } };
  assert.deepEqual(lockDrift(lock, installed), { missing: ['vite'], changed: ['solid-js'] });
  assert.deepEqual(lockDrift(lock, { packages: { ...installed.packages, 'node_modules/@wisp/ui': { link: true }, 'node_modules/solid-js': { version: '1.9.5' }, 'node_modules/vite': { version: '7.0.0' } } }), { missing: [], changed: [] });
});

test('a_workspace_pulled_without_npm_install_is_out_of_sync', () => {
  const lock = { packages: { 'node_modules/@wisp-plugin/x-ui': { resolved: 'plugins/x/ui', link: true }, 'node_modules/@wisp/ui': { resolved: 'packages/ui', link: true } } };
  const installed = { packages: { 'node_modules/@wisp/ui': { resolved: 'packages/ui', link: true } } };
  assert.deepEqual(lockDrift(lock, installed), { missing: ['@wisp-plugin/x-ui'], changed: [] });
  const moved = { packages: { ...installed.packages, 'node_modules/@wisp-plugin/x-ui': { resolved: 'plugins/old/ui', link: true } } };
  assert.deepEqual(lockDrift(lock, moved).missing, ['@wisp-plugin/x-ui'], 'a link to another folder');
});

test('deps_missing_or_out_of_sync_block_with_npm_install', () => {
  assert.equal(depsCheck(false, undefined).status, 'fail');
  const c = depsCheck(true, { missing: ['a', 'b'], changed: ['c', 'd'] });
  assert.equal(c.status, 'fail');
  assert.equal(c.fix, 'npm install');
  assert.match(c.detail, /4 out of sync.*\+1 more/);
  assert.equal(depsCheck(true, { missing: [], changed: [] }).status, 'ok');
  assert.equal(depsCheck(true, undefined).status, 'warn', 'no hidden lockfile: advice, not a block');
});

const facts = (o: Partial<ToolchainFacts> = {}): ToolchainFacts => ({
  rustup: 'rustup 1.29.3', pinned: '1.100.0', installed: ['stable-aarch64-apple-darwin', '1.100.0-aarch64-apple-darwin'],
  override: undefined, components: ['clippy-aarch64-apple-darwin', 'rustfmt-aarch64-apple-darwin', 'rustc-aarch64-apple-darwin'],
  wantComponents: ['clippy', 'rustfmt'], ...o,
});
const byId = (cs: { id: string; status: string }[], id: string): string | undefined => cs.find((c) => c.id === id)?.status;

test('the_pinned_toolchain_with_its_components_passes', () => {
  const cs = toolchainChecks(facts());
  assert.deepEqual(cs.map((c) => [c.id, c.status]), [['rustup', 'ok'], ['toolchain', 'ok'], ['components', 'ok']]);
});

test('no_rustup_blocks_a_missing_toolchain_or_component_is_advice', () => {
  assert.equal(byId(toolchainChecks(facts({ rustup: undefined })), 'rustup'), 'fail');
  const missing = toolchainChecks(facts({ installed: ['stable-aarch64-apple-darwin'], components: undefined }));
  assert.equal(byId(missing, 'toolchain'), 'warn', 'the first cargo call installs it');
  assert.equal(byId(missing, 'components'), undefined, 'no component check without the toolchain');
  const noClippy = toolchainChecks(facts({ components: ['rustfmt-aarch64-apple-darwin'] }));
  assert.equal(byId(noClippy, 'components'), 'warn');
  assert.match(noClippy.find((c) => c.id === 'components')?.fix ?? '', /rustup component add clippy --toolchain 1\.100\.0/);
});

test('rustup_toolchain_overriding_the_pin_blocks_and_the_pin_itself_does_not', () => {
  assert.equal(byId(toolchainChecks(facts({ override: 'stable-aarch64-apple-darwin' })), 'rustup-override'), 'fail');
  assert.equal(byId(toolchainChecks(facts({ override: '1.100.0' })), 'rustup-override'), undefined);
  assert.equal(byId(toolchainChecks(facts({ override: '1.100.0-aarch64-apple-darwin' })), 'rustup-override'), undefined);
  assert.equal(byId(toolchainChecks(facts({ override: '1.100' })), 'rustup-override'), 'fail', '1.100 is not 1.100.0');
});

test('an_inherited_override_at_the_pinned_version_is_advice_not_a_block', () => {
  const same = toolchainChecks(facts({ override: 'stable-aarch64-apple-darwin', overrideVersion: '1.100.0' }));
  assert.equal(byId(same, 'rustup-override'), 'warn');
  assert.equal(byId(toolchainChecks(facts({ override: 'stable', overrideVersion: '1.101.0' })), 'rustup-override'), 'fail', 'another version blocks');
  assert.equal(byId(toolchainChecks(facts({ override: 'nightly', overrideVersion: undefined })), 'rustup-override'), 'fail', 'unknown blocks');
  assert.equal(exitCode(same), 0);
});

test('rust_toolchain_toml_is_read', () => {
  const t = '# c\n[toolchain]\nchannel = "1.100.0"\ncomponents = ["clippy", "rustfmt"]\nprofile = "minimal"\n';
  assert.deepEqual(parseToolchainToml(t), { channel: '1.100.0', components: ['clippy', 'rustfmt'] });
  assert.deepEqual(parseToolchainToml(''), { channel: undefined, components: [] });
});

test('monoflow_hooks_block_other_hooks_are_a_note_missing_monoflow_is_advice', () => {
  assert.equal(hooksCheck(['.git/hooks/pre-commit (a git shield hook)'], [], undefined).status, 'fail');
  assert.equal(hooksCheck([], ['pre-push'], '.githooks').status, 'info');
  assert.equal(hooksCheck([], [], undefined).status, 'ok');
  assert.equal(monoflowCheck(false, false).status, 'warn');
  assert.match(monoflowCheck(false, false).fix ?? '', /setup-monoflow\.ts.*--build/);
  assert.equal(monoflowCheck(true, true).status, 'ok');
});

test('desktop_prereqs_block_only_on_macos_where_cargo_needs_the_clt', () => {
  assert.equal(desktopCheck('darwin', false).status, 'fail');
  assert.equal(desktopCheck('linux', false).status, 'warn');
  assert.equal(desktopCheck('win32', false).status, 'warn');
  assert.equal(desktopCheck('linux', undefined).status, 'info');
  assert.match(desktopCheck('linux', false).fix ?? '', /webkit2gtk-4\.1/);
  assert.equal(xcodeCheck('linux', false), undefined, 'xcode is a macOS check');
  assert.equal(xcodeCheck('darwin', false)?.status, 'info', 'only mobile/ios needs it');
  assert.match(xcodeCheck('darwin', false)?.detail ?? '', /mobile\/ios/);
});

test('only_fail_lines_exit_1', () => {
  const warn = fakeAgentsCheck([], ['wisp-fake-agent'], '/t/debug');
  assert.equal(warn.status, 'warn');
  assert.equal(exitCode([warn, diskCheck(1, '/t'), xcodeCheck('darwin', false) ?? warn]), 0);
  assert.equal(exitCode([warn, nodeCheck('v18.0.0', '>=22.18')]), 1);
});

test('the_report_has_a_line_per_check_the_fix_under_it_and_a_summary', () => {
  const text = format([nodeCheck('v24.0.0', '>=22.18'), nodeCheck('v18.0.0', '>=22.18'), diskCheck(1, '/t')], 12);
  const lines = text.trimEnd().split('\n');
  assert.match(lines[0] ?? '', /^ok {4}Node/);
  assert.match(lines[1] ?? '', /^FAIL {2}Node/);
  assert.match(lines[2] ?? '', /→ install Node/);
  assert.match(lines[3] ?? '', /^warn {2}Free disk/);
  assert.match(lines.at(-1) ?? '', /1 blocking, 1 advice.*12 ms/);
  assert.doesNotMatch(format([nodeCheck('v24.0.0', '>=22.18')], 1), /→/, 'an ok line has no fix');
});
