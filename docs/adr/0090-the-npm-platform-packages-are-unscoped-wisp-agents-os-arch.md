# ADR-0090: The npm platform packages are unscoped `wisp-agents-<os>-<arch>`

- Status: accepted
- Date: 2026-11-16
- Source: release.yml's v2 npm job (docs/v2-plan.md §4.7). Amends ADR-0013's *Distribution* line: the package names
  only; the launcher + per-platform `optionalDependencies` model (esbuild's) is unchanged.

## Context

ADR-0013 named the per-platform binary packages `@wisp/core-<os>-<arch>`. The `@wisp` scope on npm belongs to
another organisation (it holds `@wisp/antd`, `@wisp/admin-components`), so wisp can never publish under it. The
`@wisp/*` names in this repo's UI workspaces are private workspaces and never published, so they don't collide.

## Decision

- Each platform package is unscoped and named after the launcher: `wisp-agents-<process.platform>-<process.arch>`
  (`wisp-agents-darwin-arm64`, `wisp-agents-linux-x64`, `wisp-agents-win32-x64`, …), one per
  `npm/wisp-agents/lib/resolve.js` `TARGETS` entry. They were all free on the registry on 2026-11-16.
- They are published from the npm account that maintains `wisp-agents`, by `release.yml` with trusted publishing.
  npm can only trust a package that exists, so the **first publish of each new platform package is by hand, by the
  owner**, then trusted (the `release` skill). Agents never do it.
- The name lives in one place (`packageName` in `lib/resolve.js`); `npm/platform/package.template.json`,
  `build.mjs`'s output folders and `release.mjs`'s publish order follow it, and `npm/test/release.test.mjs` refuses
  any scoped name.
- The update plugin finds a hoisted install's launcher from the platform package folder by this name
  (`plugins/update/core/src/install.rs` `launcher_root`).

## Consequences

- No published wisp package depends on a scope we don't own. `npm view wisp-agents-<target>` shows ours.
- A new target is a new package name: its first publish is by hand before release.yml can publish it.
- Edge cases, each with its test: a scoped or non-matching name in the launcher's pins
  (`npm/test/release.test.mjs` `platform_packages_are_unscoped_wisp_agents_os_arch`); a folder in npm/dist that no
  target builds is refused, not published (`publish_order_puts_every_platform_package_before_the_launcher`); the
  launcher's own tarball is never mistaken for a platform one (release.yml checks tarballs by target name, not by a
  `wisp-agents-*` glob); a hoisted `node_modules/wisp-agents-<os>-<arch>/bin/wisp` still finds
  `node_modules/wisp-agents` as its install root (`plugins/update/core/tests/updater.rs`
  `install_method_tells_a_clone_npx_a_keg_an_app_and_a_global_install_apart`), while an unrelated
  `node_modules/wisp-agents-foo` package is not taken for one
  (`a_hoisted_binary_is_found_only_in_a_platform_package_folder`).
