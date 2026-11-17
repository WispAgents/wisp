---
name: release
description: Use when cutting a wisp v2 release - bumping the version of the published package and letting release.yml publish npm, the desktop app, the GitHub release and the Homebrew tap.
---

# Release

v2 ships on npm as `wisp-agents` only (ADR-0094): the launcher (a dependency-free script with the web build in
`web/`) and, per target, the Rust `wisp` binary as a version of `wisp-agents` itself, `<version>-<os>-<arch>` under
the dist-tag `platform-<os>-<arch>` (`npm/README.md`). The launcher installs each through an npm alias
(`wisp-agents-<os>-<arch>`: `npm:wisp-agents@<version>-<os>-<arch>`), pinned to its own version, so they all publish
together, and wisp-agents' one trusted publisher covers every tarball: nothing is ever published by hand.

1. Releasing is merging a PR that bumps the version in **both** `Cargo.toml` (`[workspace.package] version`: what
   `wisp --version` prints; then `cargo update --workspace` so `Cargo.lock` follows) and
   `npm/wisp-agents/package.json` (`version` and every `npm:wisp-agents@<version>-<os>-<arch>` pin in `optionalDependencies`).
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
     `wisp-bin-<os>-<arch>.tar.gz` for the machines plugin's installer), **npm** (those exact tarballs, `--provenance`: every binary version
     first under `platform-<os>-<arch>`, the launcher last under its channel's tag, each version npm already has skipped), **publish** (the release goes
     public), **homebrew** (the tap points at the `wisp-agents` npm tarball; npm brings the platform package).
   A run that stopped half way is finished by re-running it (Actions → release → Run workflow).
5. A version with `-` (`2.0.0-beta.1`) is a beta: npm `next`, a prerelease, no Homebrew. Nightlies are
   `<next patch>-nightly.<date>.<time>` on npm `nightly`, stamped into `Cargo.toml` and every npm package.
6. Try the whole build on a branch without publishing: `gh workflow run release --ref <branch> -f dry_run=true`
   (the tested tarballs are the run's `npm-packages` artifact).

npm trusted publishing (OIDC) is set up on npmjs.com for `wisp-agents`: repo WispAgents/wisp, workflow
`release.yml`, environment `npm`. Every tarball is a version of that one package (ADR-0094), so it covers them all,
a new target included: nothing is published by hand and there is no npm token.
