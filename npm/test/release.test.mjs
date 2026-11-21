import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { TARGETS, packageName } from '../wisp-agents/lib/resolve.js';
import { publishTag, buildMatrix, cargoVersion, extraTags, isPrerelease, lockVersion, npmTag, publishOrder, releaseKind, stampCargoVersion, versionProblems } from '../scripts/release.mjs';

const SCRIPT = fileURLToPath(new URL('../scripts/release.mjs', import.meta.url));
const CARGO = `[workspace]
members = ["crates/*"]

[workspace.package]
version = "2.1.0"
edition = "2024"
rust-version = "1.89"

[workspace.dependencies]
serde = { version = "1.0.200" }
`;
const pins = (v) => Object.fromEntries(Object.keys(TARGETS).map((t) => [packageName(...t.split('-')), `npm:wisp-agents@${v}-${t}`]));
const launcher = (v, deps = pins(v)) => ({ name: 'wisp-agents', version: v, optionalDependencies: deps });

test('cargo_version_reads_only_the_workspace_package_table', () => {
  assert.equal(cargoVersion(CARGO), '2.1.0');
  assert.equal(cargoVersion(CARGO.replace(/\n/g, '\r\n')), '2.1.0');
  // a dependency's version, or rust-version, is never taken for the workspace's
  assert.equal(cargoVersion('[workspace.package]\nrust-version = "1.89"\n\n[workspace.dependencies]\nversion = "9.9.9"\n'), null);
  assert.equal(cargoVersion('[package]\nversion = "1.0.0"\n'), null);
});

test('stamp_cargo_version_changes_the_workspace_version_and_nothing_else', () => {
  const out = stampCargoVersion(CARGO, '2.1.1-nightly.20261116.930');
  assert.equal(cargoVersion(out), '2.1.1-nightly.20261116.930');
  assert.equal(out, CARGO.replace('version = "2.1.0"', 'version = "2.1.1-nightly.20261116.930"'));
  assert.throws(() => stampCargoVersion(CARGO, 'nope'), /not a semver/);
  assert.throws(() => stampCargoVersion('[workspace]\n', '1.0.0'), /no version/);
});

test('lock_version_reads_the_wisp_crate_only', () => {
  const lock = '[[package]]\nname = "wisp-kernel"\nversion = "9.0.0"\n\n[[package]]\nname = "wisp"\nversion = "2.1.0"\ndependencies = []\n';
  assert.equal(lockVersion(lock), '2.1.0');
  assert.equal(lockVersion(lock.replace(/\n/g, '\r\n')), '2.1.0');
  assert.equal(lockVersion('[[package]]\nname = "wisp-kernel"\nversion = "9.0.0"\n'), null);
});

test('a_cargo_lock_behind_cargo_toml_says_to_run_cargo_update', () => {
  const [p] = versionProblems({ cargo: '2.1.0', lock: '2.0.0', launcher: launcher('2.1.0') });
  assert.match(p, /Cargo\.lock has wisp 2\.0\.0 but Cargo\.toml says 2\.1\.0: run `cargo update --workspace`/);
  assert.match(versionProblems({ cargo: '2.1.0', lock: null, launcher: launcher('2.1.0') })[0], /Cargo\.lock has wisp \(none\)/);
  assert.deepEqual(versionProblems({ cargo: '2.1.0', lock: '2.1.0', launcher: launcher('2.1.0') }), []);
});

test('versions_that_agree_have_no_problems', () => {
  assert.deepEqual(versionProblems({ cargo: '2.1.0', launcher: launcher('2.1.0') }), []);
  assert.deepEqual(versionProblems({ cargo: '2.1.0', launcher: launcher('2.1.0'), tag: 'v2.1.0' }), []);
  assert.deepEqual(versionProblems({ cargo: '2.1.0-beta.1', launcher: launcher('2.1.0-beta.1'), tag: '2.1.0-beta.1' }), []);
});

test('a_cargo_and_npm_mismatch_names_both_versions_and_the_fix', () => {
  const [p, ...rest] = versionProblems({ cargo: '2.0.0', launcher: launcher('2.1.0') });
  assert.equal(rest.length, 0);
  assert.match(p, /Cargo\.toml.*2\.0\.0.*npm\/wisp-agents\/package\.json.*2\.1\.0.*bump both/);
});

test('a_tag_that_differs_from_the_version_fails', () => {
  const [p] = versionProblems({ cargo: '2.1.0', launcher: launcher('2.1.0'), tag: 'v2.2.0' });
  assert.match(p, /tag v2\.2\.0 does not match the version 2\.1\.0/);
});

test('missing_or_bad_versions_are_reported', () => {
  assert.match(versionProblems({ cargo: null, launcher: launcher('2.1.0') })[0], /Cargo\.toml has no/);
  assert.match(versionProblems({ cargo: '2.1.0', launcher: { optionalDependencies: pins('2.1.0') } })[0], /has no "version"/);
  assert.ok(versionProblems({ cargo: 'x', launcher: launcher('x', pins('x')) }).some((p) => /not semver/.test(p)));
});

test('optional_dependencies_must_be_every_target_pinned_to_the_version', () => {
  const deps = pins('2.1.0');
  const [first] = Object.keys(deps);
  const stale = versionProblems({ cargo: '2.1.0', launcher: launcher('2.1.0', { ...deps, [first]: '2.0.0' }) });
  const target = first.slice('wisp-agents-'.length);
  assert.match(stale.join('\n'), new RegExp(`pins ${first}: 2\\.0\\.0: pin each to npm:wisp-agents@2\\.1\\.0-${target}`));
  const { [first]: _, ...without } = deps;
  assert.match(versionProblems({ cargo: '2.1.0', launcher: launcher('2.1.0', without) }).join('\n'), new RegExp(`lack ${first}`));
  const extra = versionProblems({ cargo: '2.1.0', launcher: launcher('2.1.0', { ...deps, 'wisp-agents-aix-ppc64': '2.1.0' }) });
  assert.match(extra.join('\n'), /wisp-agents-aix-ppc64, which no target builds/);
});

test('the_matrix_builds_every_launcher_target_on_its_own_os', () => {
  const m = buildMatrix();
  assert.deepEqual(m.map((e) => e.target), Object.keys(TARGETS));
  for (const e of m) {
    assert.equal(e.triple, TARGETS[e.target]);
    const os = e.target.split('-')[0];
    assert.equal(e.runner, { darwin: 'macos-latest', linux: 'ubuntu-latest', win32: 'windows-latest' }[os]);
    assert.equal(e.zig, os === 'linux');
    assert.equal(e.bin, os === 'win32' ? 'wisp.exe' : 'wisp');
    assert.equal(e.rustflags, os === 'win32' ? '-C target-feature=+crt-static' : '');
  }
  // linux is musl on every arch: static, so one package runs on glibc and musl
  for (const e of m.filter((x) => x.zig)) assert.match(e.triple, /-linux-musl/);
  assert.throws(() => buildMatrix({ 'aix-ppc64': 'powerpc64-ibm-aix' }), /No runner builds aix-ppc64/);
});

test('npm_tag_is_latest_next_or_nightly', () => {
  assert.equal(npmTag('2.1.0', 'stable'), 'latest');
  assert.equal(npmTag('2.1.0-beta.1', 'stable'), 'next');
  assert.equal(npmTag('2.1.0-rc.1', 'stable'), 'next');
  assert.equal(npmTag('2.1.1-nightly.20261116.930', 'nightly'), 'nightly');
  assert.throws(() => npmTag('2.1.0', 'canary'), /Unknown channel/);
});

test('an_alpha_is_the_public_channel_latest_on_npm_plus_alpha', () => {
  // 1.0.0-alpha.1 is what users get by default: npm latest (and alpha), not next
  assert.equal(releaseKind('1.0.0-alpha.1'), 'alpha');
  assert.equal(npmTag('1.0.0-alpha.1', 'stable'), 'latest');
  assert.deepEqual(extraTags('1.0.0-alpha.1', 'stable'), ['alpha']);
  assert.equal(npmTag('1.0.0-alpha.12', 'stable'), 'latest');
  // only `alpha.<n>` is: anything else with a `-` stays a beta
  for (const v of ['1.0.0-alpha', '1.0.0-alpha.1.2', '1.0.0-alphabet.1', '1.0.0-beta.1', '1.0.0-rc.2']) {
    assert.equal(releaseKind(v), 'beta', v);
    assert.equal(npmTag(v, 'stable'), 'next', v);
    assert.deepEqual(extraTags(v, 'stable'), [], v);
  }
  assert.equal(releaseKind('1.0.0'), 'stable');
  assert.deepEqual(extraTags('1.0.0', 'stable'), []);
  assert.deepEqual(extraTags('1.0.1-nightly.20261121.930', 'nightly'), []);
  assert.throws(() => releaseKind('nope'), /not a semver/);
  assert.throws(() => extraTags('1.0.0', 'canary'), /Unknown channel/);
});

test('only_a_beta_or_a_nightly_is_a_github_prerelease', () => {
  // the plan job's `prerelease`: --prerelease and --latest=false on the release, and no Homebrew
  assert.equal(isPrerelease('1.0.0', 'stable'), false);
  assert.equal(isPrerelease('1.0.0-alpha.1', 'stable'), false);
  assert.equal(isPrerelease('1.0.0-beta.1', 'stable'), true);
  assert.equal(isPrerelease('1.0.1-nightly.20261121.930', 'nightly'), true);
  assert.throws(() => isPrerelease('1.0.0', 'canary'), /Unknown channel/);
});

test('the_cli_says_an_alphas_tags_and_that_it_is_no_prerelease', () => {
  const run = (...a) => spawnSync(process.execPath, [SCRIPT, ...a], { encoding: 'utf8' });
  assert.equal(run('npm-tag', '1.0.0-alpha.1', 'stable').stdout, 'latest\n');
  assert.equal(run('extra-tags', '1.0.0-alpha.1', 'stable').stdout, 'alpha\n');
  assert.equal(run('extra-tags', '1.0.0', 'stable').stdout, '\n');
  assert.equal(run('prerelease', '1.0.0-alpha.1', 'stable').stdout, 'false\n');
  assert.equal(run('prerelease', '1.0.0-beta.1', 'stable').stdout, 'true\n');
  assert.equal(run('prerelease', '1.0.1-nightly.20261121.930', 'nightly').stdout, 'true\n');
});

test('every_binary_is_a_version_of_wisp_agents_under_its_alias', () => {
  // ADR-0094: one package name on npm (its trusted publisher covers every tarball); the launcher's alias keeps the
  // folder the launcher looks in (wisp-agents-<os>-<arch>)
  for (const t of Object.keys(TARGETS)) assert.equal(packageName(...t.split('-')), `wisp-agents-${t}`);
  const pkg = JSON.parse(fs.readFileSync(new URL('../wisp-agents/package.json', import.meta.url), 'utf8'));
  for (const [name, spec] of Object.entries(pkg.optionalDependencies)) {
    assert.match(name, /^wisp-agents-[a-z0-9]+-[a-z0-9]+$/);
    assert.equal(spec, `npm:wisp-agents@${pkg.version}-${name.slice('wisp-agents-'.length)}`);
  }
  const template = fs.readFileSync(new URL('../platform/package.template.json', import.meta.url), 'utf8');
  assert.match(template, /"name": "wisp-agents",/);
  assert.match(template, /"version": "\$\{version\}-\$\{os\}-\$\{arch\}"/);
});

test('a_binary_goes_out_under_its_platform_tag_and_never_latest', () => {
  assert.equal(publishTag('2.1.0', '2.1.0', 'latest'), 'latest');
  assert.equal(publishTag('2.1.0-linux-x64', '2.1.0', 'latest'), 'platform-linux-x64');
  assert.equal(publishTag('2.2.0-beta.1-darwin-arm64', '2.2.0-beta.1', 'next'), 'platform-darwin-arm64');
  assert.equal(publishTag('2.1.1-nightly.20261117.930-win32-x64', '2.1.1-nightly.20261117.930', 'nightly'), 'platform-win32-x64');
  assert.throws(() => publishTag('2.0.0-linux-x64', '2.1.0', 'latest'), /neither this release/);
  assert.throws(() => publishTag('2.1.0-aix-ppc64', '2.1.0', 'latest'), /neither this release/);
});

test('publish_order_puts_every_platform_package_before_the_launcher', () => {
  const core = Object.keys(TARGETS).map((t) => `/d/wisp-agents-${t}`);
  const order = publishOrder(['/d/wisp-agents', ...core.slice().reverse()]);
  assert.equal(order.at(-1), '/d/wisp-agents');
  assert.deepEqual(order.slice(0, -1), core.slice().sort());
  assert.throws(() => publishOrder(core), /exactly one wisp-agents/);
  assert.throws(() => publishOrder(['/d/wisp-agents', ...core.slice(1)]), new RegExp(`No package built for wisp-agents-${Object.keys(TARGETS)[0]}`));
  assert.throws(() => publishOrder(['/d/wisp-agents', '/d/wisp-agents-aix-ppc64', ...core]), /wisp-agents-aix-ppc64 is not a package/);
});

test('the_checked_in_versions_agree_and_check_prints_the_version', () => {
  const r = spawnSync(process.execPath, [SCRIPT, 'check'], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  const want = JSON.parse(fs.readFileSync(new URL('../wisp-agents/package.json', import.meta.url), 'utf8')).version;
  assert.equal(r.stdout.trim(), want);
  const bad = spawnSync(process.execPath, [SCRIPT, 'check', '--tag', 'v0.0.1'], { encoding: 'utf8' });
  assert.equal(bad.status, 1);
  assert.match(bad.stderr, /^::error::The tag v0\.0\.1 does not match/m);
});

test('the_matrix_command_prints_one_line_of_json', () => {
  const r = spawnSync(process.execPath, [SCRIPT, 'matrix'], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout.trim().split('\n').length, 1);
  assert.deepEqual(JSON.parse(r.stdout), buildMatrix());
});
