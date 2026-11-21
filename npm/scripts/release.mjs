#!/usr/bin/env node
// The pure half of .github/workflows/release.yml: which version is being released and whether every
// file that carries it agrees, which runner builds which target, which npm dist-tag a release goes
// to, and the order the packages are published in. Node built-ins only, so a bare runner can run it.
//
//   node npm/scripts/release.mjs check [--tag v1.0.0]   # prints the version, or exits 1 saying what disagrees
//   node npm/scripts/release.mjs matrix                 # the binaries job's matrix (JSON, one line)
//   node npm/scripts/release.mjs npm-tag <version> <stable|nightly>     # the launcher's dist-tag (latest, next, nightly)
//   node npm/scripts/release.mjs extra-tags <version> <stable|nightly>  # dist-tags it also gets (an alpha's `alpha`), space-separated
//   node npm/scripts/release.mjs prerelease <version> <stable|nightly>  # true for a GitHub prerelease (beta, nightly)
//   node npm/scripts/release.mjs packages <dist-dir>    # package dirs in publish order, one per line
//   node npm/scripts/release.mjs publish-tag <pkg-version> <version> <tag>  # a tarball's dist-tag (a binary's: platform-<target>)
//   node npm/scripts/release.mjs stamp-cargo <version>  # a nightly's version into Cargo.toml's [workspace.package]
//   node npm/scripts/release.mjs set-plugins <version>  # a release's version into every built-in plugin's manifest
//                                                       # (then `cargo update --workspace` moves Cargo.lock with it)

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PREFIX, TARGETS, binaryName, packageName, platformSpec, platformTag, platformVersion } from '../wisp-agents/lib/resolve.js';

const ROOT = fileURLToPath(new URL('../..', import.meta.url));
const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/;

// The `version = "…"` line of Cargo.toml's [workspace.package] table: the version every crate,
// and so `wisp --version`, carries. Only that table: a dependency's version must never match.
const WORKSPACE_PACKAGE = /^\[workspace\.package\][^\S\n]*\n(?:(?!\[)[^\n]*\n)*?version[^\S\n]*=[^\S\n]*"([^"]*)"/m;

export function cargoVersion(cargoToml) {
  const m = WORKSPACE_PACKAGE.exec(cargoToml.replace(/\r\n/g, '\n'));
  return m ? m[1] : null;
}

// The version Cargo.lock records for the `wisp` crate: a bump that skipped `cargo update --workspace`
// fails every --locked build late, so the plan job checks it first.
export function lockVersion(cargoLock) {
  const m = /^\[\[package\]\]\nname = "wisp"\nversion = "([^"]*)"/m.exec(cargoLock.replace(/\r\n/g, '\n'));
  return m ? m[1] : null;
}

export function stampCargoVersion(cargoToml, version) {
  if (!SEMVER.test(version)) throw new Error(`"${version}" is not a semver version.`);
  const text = cargoToml.replace(/\r\n/g, '\n');
  const m = WORKSPACE_PACKAGE.exec(text);
  if (!m) throw new Error('Cargo.toml has no version in [workspace.package].');
  const end = m.index + m[0].length;
  return `${text.slice(0, end - m[1].length - 1)}${version}"${text.slice(end)}`;
}

// Every built-in plugin's manifest version, as { file: version }: they carry the product's version, so Settings →
// Plugins says the version you installed (bumped with it, by hand or `set-plugins <version>`).
export function manifestVersions(root = ROOT) {
  const out = {};
  for (const p of fs.readdirSync(path.join(root, 'plugins'))) {
    const file = path.join('plugins', p, 'host', 'wisp-plugin.json');
    if (!fs.existsSync(path.join(root, file))) continue;
    out[file] = JSON.parse(fs.readFileSync(path.join(root, file), 'utf8')).version ?? null;
  }
  return out;
}

// A manifest with its top-level "version" set (the first "version" key at two-space indent: a `provides` entry's
// own version is deeper and stays).
export function stampManifestVersion(json, version) {
  if (!SEMVER.test(version)) throw new Error(`"${version}" is not a semver version.`);
  const text = json.replace(/\r\n/g, '\n');
  if (!/^ {2}"version": "[^"]*"/m.test(text)) throw new Error('the manifest has no top-level "version".');
  return text.replace(/^( {2}"version": ")[^"]*(")/m, `$1${version}$2`);
}

// Every problem with the versions, as sentences that say what to fix; [] when they agree.
// tag: the git tag being released (v1.2.3), when the run is for one.
// manifests: the built-in plugins' versions ({ file: version }), when checked.
export function versionProblems({ cargo, lock, launcher, tag, manifests }) {
  const problems = [];
  const npm = launcher?.version;
  if (!cargo) problems.push('Cargo.toml has no `version = "x.y.z"` under [workspace.package].');
  if (!npm) problems.push('npm/wisp-agents/package.json has no "version".');
  if (npm && !SEMVER.test(npm)) problems.push(`npm/wisp-agents/package.json's version "${npm}" is not semver.`);
  if (cargo && npm && cargo !== npm) {
    problems.push(`Cargo.toml's [workspace.package] version is ${cargo} but npm/wisp-agents/package.json's is ${npm}: bump both to the same version (the release skill).`);
  }
  if (cargo && lock !== undefined && lock !== cargo) {
    problems.push(`Cargo.lock has wisp ${lock ?? '(none)'} but Cargo.toml says ${cargo}: run \`cargo update --workspace\` and commit Cargo.lock.`);
  }
  const behind = Object.entries(manifests ?? {}).filter(([, v]) => cargo && v !== cargo);
  if (behind.length) {
    problems.push(
      `${behind.length} built-in plugin manifest${behind.length === 1 ? '' : 's'} (${behind.map(([f, v]) => `${f}: ${v}`).slice(0, 3).join(', ')}${behind.length > 3 ? ', …' : ''}) don't say ${cargo}: run \`node npm/scripts/release.mjs set-plugins ${cargo}\` (the release skill).`,
    );
  }
  if (tag && npm && tag.replace(/^v/, '') !== npm) {
    problems.push(`The tag ${tag} does not match the version ${npm} in npm/wisp-agents/package.json and Cargo.toml: tag v${npm}, or bump the version.`);
  }
  // build.mjs rewrites these, but the checked-in file is what a reviewer reads and what
  // `npm pack npm/wisp-agents` would ship: keep it exactly the supported set, pinned to the version.
  const want = Object.keys(TARGETS).map((t) => packageName(...t.split('-')));
  const have = launcher?.optionalDependencies ?? {};
  const missing = want.filter((n) => !Object.hasOwn(have, n));
  const extra = Object.keys(have).filter((n) => !want.includes(n));
  if (missing.length) problems.push(`npm/wisp-agents/package.json's optionalDependencies lack ${missing.join(', ')}.`);
  if (extra.length) problems.push(`npm/wisp-agents/package.json's optionalDependencies name ${extra.join(', ')}, which no target builds.`);
  // each binary is a version of wisp-agents itself, through an npm alias (ADR-0094)
  const spec = (n) => platformSpec(npm, ...n.slice(PREFIX.length + 1).split('-'));
  const stale = want.filter((n) => Object.hasOwn(have, n) && npm && have[n] !== spec(n));
  if (stale.length) problems.push(`npm/wisp-agents/package.json pins ${stale.map((n) => `${n}: ${have[n]}`).join(', ')}: pin each to ${stale.map(spec).join(', ')}.`);
  return problems;
}

// Which runner builds each target. Mac and Windows build their targets natively (the arm64 ones
// cross-compile on the same OS, as the desktop job does). Linux is static musl for every arch,
// cross-compiled on one x64 runner with cargo-zigbuild, which brings the C toolchain the
// sqlite, ring, aws-lc and zstd builds need for each musl triple.
const RUNNERS = { darwin: 'macos-latest', linux: 'ubuntu-latest', win32: 'windows-latest' };

export function buildMatrix(targets = TARGETS) {
  return Object.entries(targets).map(([target, triple]) => {
    const os = target.split('-')[0];
    const runner = RUNNERS[os];
    if (!runner) throw new Error(`No runner builds ${target} (${triple}): add its OS to RUNNERS in npm/scripts/release.mjs.`);
    // Windows links the C runtime statically, so wisp.exe runs where the VC++ redistributable isn't installed.
    const rustflags = os === 'win32' ? '-C target-feature=+crt-static' : '';
    return { target, triple, runner, zig: os === 'linux', bin: binaryName(os), rustflags };
  });
}

// What a stable-channel version's release is. Everything else follows from it:
//   x.y.z          `stable`: npm `latest`, the GitHub release marked Latest, Homebrew
//   x.y.z-alpha.N  `alpha`, the public channel while wisp is in alpha: what users get by default, so the same as
//                  stable (npm `latest`, Latest on GitHub, so /releases/latest and the app's updater see it,
//                  Homebrew), plus the npm dist-tag `alpha` that names the newest alpha
//   x.y.z-<other>  `beta` (beta.N, rc.N, …): npm `next`, a GitHub prerelease, no Homebrew
// Nightlies are their own channel (npm `nightly`, the rolling `nightly` prerelease) and never come here.
const ALPHA = /^\d+\.\d+\.\d+-alpha\.\d+$/;

export function releaseKind(version) {
  if (!SEMVER.test(version)) throw new Error(`"${version}" is not a semver version.`);
  if (!version.includes('-')) return 'stable';
  return ALPHA.test(version) ? 'alpha' : 'beta';
}

// A GitHub prerelease (not Latest, no Homebrew): a beta or a nightly, never a stable or an alpha release.
export function isPrerelease(version, channel) {
  if (channel === 'nightly') return true;
  if (channel !== 'stable') throw new Error(`Unknown channel "${channel}" (stable or nightly).`);
  return releaseKind(version) === 'beta';
}

// npm dist-tag the launcher is published under: nightlies on `nightly`, a beta on `next`, a stable or alpha
// release on `latest`.
export function npmTag(version, channel) {
  if (channel === 'nightly') return 'nightly';
  if (channel !== 'stable') throw new Error(`Unknown channel "${channel}" (stable or nightly).`);
  return releaseKind(version) === 'beta' ? 'next' : 'latest';
}

// The dist-tags the launcher also gets once it's published (`npm publish` takes one --tag): an alpha's `alpha`.
export function extraTags(version, channel) {
  if (channel === 'nightly') return [];
  if (channel !== 'stable') throw new Error(`Unknown channel "${channel}" (stable or nightly).`);
  return releaseKind(version) === 'alpha' ? ['alpha'] : [];
}

// The dist-tag a packed tarball goes out under: the launcher (`version`) under the release's own tag; a target's binary
// (`<version>-<os>-<arch>`, ADR-0094) under platform-<os>-<arch>, so latest, next and nightly only ever name the
// launcher. Anything else is refused: it isn't this release's.
export function publishTag(pkgVersion, version, tag) {
  if (pkgVersion === version) return tag;
  const target = Object.keys(TARGETS).find((t) => pkgVersion === platformVersion(version, ...t.split('-')));
  if (!target) throw new Error(`wisp-agents@${pkgVersion} is neither this release (${version}) nor one of its binaries.`);
  return platformTag(...target.split('-'));
}

// Every platform package (wisp-agents-<os>-<arch>, build.mjs's folder names) before wisp-agents: the
// launcher pins them all exactly, so it goes last, and a run that stops half way is finished by
// re-running (each published version is skipped). A folder that is neither is refused, not published.
export function publishOrder(dirs) {
  const want = Object.keys(TARGETS).map((t) => packageName(...t.split('-')));
  const platform = dirs.filter((d) => want.includes(path.basename(d))).sort();
  const launcher = dirs.filter((d) => path.basename(d) === PREFIX);
  if (launcher.length !== 1) throw new Error('Expected exactly one wisp-agents package to publish.');
  const stray = dirs.filter((d) => !platform.includes(d) && !launcher.includes(d)).map((d) => path.basename(d));
  if (stray.length) throw new Error(`${stray.join(', ')} is not a package release.yml publishes (no target builds it).`);
  const have = platform.map((d) => path.basename(d));
  const missing = want.filter((n) => !have.includes(n));
  if (missing.length) throw new Error(`No package built for ${missing.join(', ')}: wisp-agents would pin a version npm doesn't have.`);
  return [...platform, ...launcher];
}

const read = (rel) => fs.readFileSync(path.join(ROOT, rel), 'utf8');

function main([cmd, ...args]) {
  switch (cmd) {
    case 'check': {
      const i = args.indexOf('--tag');
      const tag = i >= 0 ? args[i + 1] : undefined;
      const launcher = JSON.parse(read('npm/wisp-agents/package.json'));
      const problems = versionProblems({ cargo: cargoVersion(read('Cargo.toml')), lock: lockVersion(read('Cargo.lock')), launcher, tag, manifests: manifestVersions() });
      if (problems.length) {
        for (const p of problems) process.stderr.write(`::error::${p}\n`);
        return 1;
      }
      process.stdout.write(`${launcher.version}\n`);
      return 0;
    }
    case 'matrix':
      process.stdout.write(`${JSON.stringify(buildMatrix())}\n`);
      return 0;
    case 'npm-tag':
      process.stdout.write(`${npmTag(args[0], args[1])}\n`);
      return 0;
    case 'extra-tags':
      process.stdout.write(`${extraTags(args[0], args[1]).join(' ')}\n`);
      return 0;
    case 'prerelease':
      process.stdout.write(`${isPrerelease(args[0], args[1])}\n`);
      return 0;
    case 'packages': {
      const dist = path.resolve(args[0] ?? path.join(ROOT, 'npm/dist'));
      const dirs = fs.readdirSync(dist).map((n) => path.join(dist, n)).filter((d) => fs.existsSync(path.join(d, 'package.json')));
      process.stdout.write(`${publishOrder(dirs).join('\n')}\n`);
      return 0;
    }
    case 'publish-tag':
      process.stdout.write(`${publishTag(args[0], args[1], args[2])}\n`);
      return 0;
    case 'set-plugins':
      // a release bump: every built-in plugin's manifest says the new version (never a nightly's: built-ins require
      // each other with ranges a nightly pre-release of another patch wouldn't satisfy)
      for (const file of Object.keys(manifestVersions())) fs.writeFileSync(path.join(ROOT, file), stampManifestVersion(read(file), args[0]));
      return 0;
    case 'stamp-cargo':
      fs.writeFileSync(path.join(ROOT, 'Cargo.toml'), stampCargoVersion(read('Cargo.toml'), args[0]));
      return 0;
    default:
      process.stderr.write('Usage: node npm/scripts/release.mjs check [--tag vX.Y.Z] | matrix | npm-tag <version> <channel> | extra-tags <version> <channel> | prerelease <version> <channel> | packages <dist> | stamp-cargo <version> | set-plugins <version>\n');
      return 2;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (err) {
    process.stderr.write(`::error::release: ${err.message}\n`);
    process.exitCode = 1;
  }
}
