# ADR-0094: Every binary is a version of wisp-agents, so trusted publishing covers the whole release

- Status: accepted
- Date: 2026-11-17
- Source: the owner, 2026-11-17 ("we have workflows to publish to npm, we don't do them manually"; "Yes: ship the
  platform binaries as versions of wisp-agents so the existing trusted publisher covers everything"). Supersedes
  ADR-0090 (seven new package names, each published once by hand). Keeps ADR-0013's launcher and ADR-0090's reason
  for no `@wisp` scope.

## Context

ADR-0090 shipped each target's `wisp` binary as its own unscoped npm package, `wisp-agents-<os>-<arch>`. npm's trusted
publishing (OIDC from `release.yml`, no token) can only be set on a package that already exists, so each of the seven
new names needed a first publish by hand from the owner's npm account before any release could go out. On 2026-11-17
all seven were still missing on npm (E404) and the repo holds no npm token: the first v2 release would have failed at
its npm step. The owner wants releases to go through the workflow only, never by hand.

## Decision

1. **One package name.** Each target's binary is published as a version of `wisp-agents` itself:
   `<version>-<os>-<arch>` (2.0.0-darwin-arm64, 2.1.0-beta.1-linux-x64, 2.0.1-nightly.20261117.930-win32-x64).
   `wisp-agents` already has its trusted publisher (WispAgents/wisp, `release.yml`, environment `npm`), and it covers
   every version: nothing is ever published by hand, and no token exists.
2. **The launcher installs its binary through an npm alias.** `npm/wisp-agents/package.json`'s optionalDependencies
   map `wisp-agents-<os>-<arch>` to `npm:wisp-agents@<version>-<os>-<arch>`. Each binary's `package.json` keeps its
   `os` and `cpu`, so npm, pnpm and yarn install only the one that matches the machine, into
   `node_modules/wisp-agents-<os>-<arch>`: the folder the launcher (`lib/resolve.js`) and the updater already look in,
   so neither changes.
3. **Dist-tags.** A binary version goes out under `platform-<os>-<arch>`, never `latest`, `next` or `nightly`, which
   only ever name the launcher (`npm/scripts/release.mjs publishTag`; a tarball that is neither this release nor one
   of its binaries stops the run). A binary version is a prerelease of its release's version, so a range such as
   `^2.0.0` never resolves to one.
4. **Order** is unchanged: every binary first, the launcher last, a re-run skipping what npm already has.

## Consequences

- A release is merging its version bump: the workflow publishes all eight tarballs with provenance. No owner step.
- wisp-agents' version list on npmjs.com shows seven extra versions per release (the binaries). `npm i wisp-agents`,
  `npx wisp-agents`, `npm i -g wisp-agents` and the `latest` tag behave as before.
- The tarball names change: `wisp-agents-<version>-<os>-<arch>.tgz` (`release.yml`'s checks follow).
- ADR-0090's names `wisp-agents-<os>-<arch>` are never published; they live on only as the alias folders.

## Places it reaches (ADR-0040)

`npm install -g wisp-agents`, `npx wisp-agents`, pnpm and yarn installs, the Homebrew formula (it installs the
launcher's tarball), `wisp update` (it reinstalls `wisp-agents`), PR CI (`npm/test`), the release workflow.

## Edge cases and their tests

- The template makes `wisp-agents@<version>-<os>-<arch>` with its os/cpu; the launcher pins every target through its
  alias; a win32 target gets `bin/wisp.exe`: `npm/test/build.test.mjs`.
- The checked-in launcher pins every target to `npm:wisp-agents@<version>-<target>`; a stale or missing pin is named
  with its fix; a binary's tag is `platform-<target>` for stable, beta and nightly, and anything else is refused:
  `npm/test/release.test.mjs` (`every_binary_is_a_version_of_wisp_agents_under_its_alias`,
  `a_binary_goes_out_under_its_platform_tag_and_never_latest`).
- Checked by hand on 2026-11-17 against a local registry serving the eight packed tarballs (version 9.9.9, a stub
  binary): `npm install wisp-agents`, `npm install -g wisp-agents` and `pnpm add wisp-agents` each installed the
  launcher plus only `wisp-agents-darwin-arm64` on an Apple-silicon Mac, and `wisp --version` ran it.
