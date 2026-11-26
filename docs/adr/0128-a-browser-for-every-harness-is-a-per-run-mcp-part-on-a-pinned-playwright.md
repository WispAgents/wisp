# ADR-0128: a browser for every harness is a per-run MCP part on a pinned Playwright

- Status: accepted
- Date: 2026-11-26
- Source: audit browser r1 (user-browser-tools, user-browser-sandbox, browser-chrome-tools-each-need-approval).

## Context

v1 gave Claude Code its browser by flag: `--chrome` when the user's own `/chrome` choice
(`claudeInChromeDefaultEnabled` in `.claude.json`, or `WISP_CLAUDE_CHROME`) said so, since Claude in Chrome turns
itself on only in interactive sessions and `-p` needs the flag. v2's launch never pushed it, so a run had no browser
at all, and no other harness ever had one. In a sandbox, v1 added the image's `wisp-browser-mcp` (Playwright's MCP
server over CDP to the box's Chromium, the one the user watches) as a `browser` connector; nothing in v2 refers to it,
and the image installed `@playwright/mcp` unpinned. When Claude in Chrome was on in Edits, every navigate, click and
screenshot went through wisp's approval card, which named it only as "use claude-in-chrome · computer".

## Decision

1. **One setting, one override.** `runtime.browser` (`default | off | chrome | headless`, Settings → Agents → General)
   is every harness's; `runtime.browsers` (`{ <harness>: value }`) is one harness's own, set from its detail in
   Settings → Agents or from the harness picker's Browser row. Unset (`default`) keeps v1: Claude Code follows its
   `/chrome` choice and `WISP_CLAUDE_CHROME` (read by wisp.claude at launch), the others have none.
2. **A typed field on the run.** `HarnessRun::browser` (`Browser`, `#[serde(default)]`: an old payload decodes as
   `Default`) is filled by wisp.runtime from the setting, resolved for the run's harness by
   `wisp_runtime_api::harness::browser::resolve` over its `BrowserSupport`: per-run MCP (`HarnessInfo::ask_tool`),
   its own Chrome (`HarnessInfo::own_chrome`, additive; Claude Code only) and whether this wisp is a sandbox's
   (`WISP_SANDBOX=1`). The same `BrowserSupport` goes out on `AgentStatus::browser` (additive), so the picker and
   Settings grey out exactly what a run would never get.
3. **A per-run MCP part.** When the resolved browser needs a server (headless; Chrome on a harness without its own;
   a sandbox's), wisp.runtime adds a `browser` stdio server after the connectors with `entry().or_insert`, so a
   connector the user named `browser` always wins. Outside a sandbox it is
   `npx -y @playwright/mcp@0.0.86 --headless --isolated`, plus `--browser chrome` when a system Chrome is installed
   (Playwright then drives it instead of downloading its own). It reaches Codex (`-c mcp_servers.browser.*`), opencode
   (its inline config), Copilot (`--additional-mcp-config`, `--allow-tool=browser`) and Grok (its agent definition)
   through their existing MCP paths, and Claude Code through its `--mcp-config` file. Cursor and Antigravity take no
   per-run MCP: they get no browser, and the picker says why instead of showing one as on.
4. **Claude Code.** Chrome pushes `--chrome` and adds `mcp__claude-in-chrome` to `--allowedTools` (in Edits its tools
   no longer each wait on wisp's card; everything else still goes to `--permission-prompt-tool`); off pushes
   `--no-chrome`; headless and a sandbox's push neither and use the `browser` server, already allowed as an MCP
   server key. A lean side run gets no browser flag. A browser approval that still reaches wisp names the page or the
   step ("open https://example.com", "click Sign in button in the browser").
5. **A sandbox.** With `WISP_SANDBOX=1`, every setting but off resolves to `Sandbox` for every MCP-capable harness, and
   the `browser` server is the image's `wisp-browser-mcp` (or `WISP_BROWSER_MCP_BIN`) found on `PATH`: CDP to the
   Chromium the user watches. Without it, the headless server, never with `--browser chrome` (the box is Linux with
   chromium only). Claude Code never gets `--chrome` there; the picker shows On (Chrome) as unavailable ("Claude in
   Chrome needs your own Chrome; a sandbox uses its own browser") and the run falls back to the sandbox's browser.
6. **One pinned version.** `PLAYWRIGHT_MCP` in `wisp-runtime-api` is the only place the version lives in code; the
   sandbox image installs the same one (`grid/deploy/image/Dockerfile`, a comment pointing back), and a runtime test
   (`the_sandbox_image_pins_the_same_playwright_mcp`) fails when the two differ. A bump changes both in one PR.

## Consequences

- A new npm package runs on users' computers, pinned and fetched by `npx` on first use (no new Rust dependency). An
  unpinned `npx` would run whatever npm served that day on every run.
- A harness plugin with its own browser integration sets `own_chrome` and reads `HarnessRun::browser`; the runtime
  knows no harness by name.
- Places it reaches (ADR-0040): every harness that takes MCP, inside wisp and from the harness itself (the server is
  an ordinary MCP server in its config); Settings → Agents and the harness picker; a sandbox's box.
