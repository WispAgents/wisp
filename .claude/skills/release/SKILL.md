---
name: release
description: Use when cutting a wisp v2 release - bumping the version of the published package and letting release.yml publish npm, the desktop app, the GitHub release and the Homebrew tap.
---

# Release

v2 ships on npm as `wisp-agents` (a dependency-free launcher with the web build in `web/`) plus one
unscoped `wisp-agents-<os>-<arch>` package per target holding the Rust `wisp` binary (`npm/README.md`; the `@wisp`
scope on npm belongs to someone else, so no published package uses it). The launcher pins
every platform package at exactly its own version, so they all publish together.

1. Releasing is merging a PR that bumps the version in **both** `Cargo.toml` (`[workspace.package] version`: what
   `wisp --version` prints; then `cargo update --workspace` so `Cargo.lock` follows) and
   `npm/wisp-agents/package.json` (`version` and every `wisp-agents-<os>-<arch>` pin in `optionalDependencies`).
   `node npm/scripts/release.mjs check` says whether they (and `Cargo.lock`) agree (release.yml's plan job runs
   it first and fails with the file to fix). Also bump the strings that show it: `README.md`, `docs/index.html`,
   `docs/install.sh`, `scripts/guides.mjs`.
2. Move CHANGELOG's *Unreleased* into `## [x.y.z] - YYYY-MM-DD`: `scripts/release-notes.mjs` drafts the GitHub
   release from that section (the plan job fails without it).
3. `.agents/bin/monoflow verify --all` and `npm run check` pass before the PR is opened.
4. After the merge (a push that changes `npm/wisp-agents/package.json`'s version; a `Cargo.toml` dependency edit
   releases nothing), `release.yml` sees a version npm doesn't have and:
   - **binaries**: `cargo build --release --locked -p wisp` for every target in `npm/wisp-agents/lib/resolve.js`
     `TARGETS` (Mac and Windows natively, Linux static musl through cargo-zigbuild; runners in
     `npm/scripts/release.mjs` `buildMatrix`);
   - **packages**: `npm run build` (the web bundle), `npm/scripts/build.mjs` per target, the launcher's tests
     (`node --test npm/test/`), `npm pack`, then the packed linux-x64 tarballs run `wisp --version` and
     `wisp serve` (the web UI is served from the launcher's `web/` through `WISP_WEB_DIR`);
   - **desktop**, **release** (the draft, with the desktop downloads and each binary as
     `wisp-bin-<os>-<arch>.tar.gz` for the machines plugin's installer), **npm** (those exact tarballs, `--provenance`: every `wisp-agents-<os>-<arch>`
     first, `wisp-agents` last, each `name@version` npm already has skipped), **publish** (the release goes
     public), **homebrew** (the tap points at the `wisp-agents` npm tarball; npm brings the platform package).
   A run that stopped half way is finished by re-running it (Actions → release → Run workflow).
5. A version with `-` (`2.0.0-beta.1`) is a beta: npm `next`, a prerelease, no Homebrew. Nightlies are
   `<next patch>-nightly.<date>.<time>` on npm `nightly`, stamped into `Cargo.toml` and every npm package.
6. Try the whole build on a branch without publishing: `gh workflow run release --ref <branch> -f dry_run=true`
   (the tested tarballs are the run's `npm-packages` artifact).

npm trusted publishing (OIDC) must be set up on npmjs.com for `wisp-agents` **and every `wisp-agents-<os>-<arch>`
package**: repo WispAgents/wisp, workflow `release.yml`, environment `npm`. A package without it fails its publish.

**The first publish of each new platform package is by hand, by the owner** (npm can only trust a package that
exists): all 7 before v2's first release, and any target added to `TARGETS` later. From the npm account that
maintains `wisp-agents`, take the tarball from a dry run's `npm-packages` artifact and run
`npm publish wisp-agents-<os>-<arch>-<version>.tgz --access public`, then add the trusted publisher on that package's
Settings page. Agents never do this step. Until it's done, release.yml's npm job fails, nightly included. That
version is then on npm already, so release.yml skips those packages for it (and publishes `wisp-agents`); the
hand-published tarballs carry no provenance, from the next version on every package does.
