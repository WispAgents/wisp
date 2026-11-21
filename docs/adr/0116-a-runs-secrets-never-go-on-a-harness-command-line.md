# ADR-0116: a run's secrets never go on a harness's command line

- Status: accepted
- Date: 2026-11-21
- Source: audit I setup, finding IS3 (`docs/qa/audit-I-setup.md`): a connector's secret env and the run's wisp token
  were on the harness's argv, which `ps -axo command` shows to any local user.

## Context

wisp hands every run its MCP servers: wisp's own (an HTTP server with the run's agent token in `x-wisp-token`) and the
user's connectors (a command with an `env` that may hold secrets read from `secrets.json`, or a URL with an
`Authorization` header). Claude Code got them as inline JSON (`--mcp-config '{…}'`), Codex as `-c
mcp_servers.<n>.env={…}` and `-c mcp_servers.wisp.http_headers={x-wisp-token=…}`, Copilot as inline
`--additional-mcp-config`. A process's argv is public on macOS and Linux (`ps`, `/proc/<pid>/cmdline`); its environment
and a 0600 file are its user's only. No ADR said how a runner gets config that holds a secret.

## Decision

1. **No secret value on a harness's argv.** What a run's MCP servers need (headers, env values) reaches the harness
   through a file of the run's own or the harness's environment, per harness:
   - **Claude Code:** `--mcp-config {tmp}/wisp-mcp.json` (`wisp_runtime_api::harness::MCP_CONFIG_FILE`), the same JSON
     as before, in a launch file.
   - **Copilot:** `--additional-mcp-config @{tmp}/wisp-mcp.json`.
   - **Codex** (its config is `-c` only, and `CODEX_HOME` is the user's sign-in): an HTTP server's headers as
     `env_http_headers={<header>="WISP_MCP_TOKEN_<i>_<j>"}`, a command server's env as `env_vars=["NAME"]` (codex forwards
     the variable by name), the values in codex's environment (`wisp_codex_core::mcp_env`). The header variables are
     named `…TOKEN…`, so codex's default shell policy keeps them from its agent's commands; the user's own
     `shell_environment_policy` is left as they set it. The names a process is shaped by (`PATH`, `HOME`, `USER`,
     `LOGNAME`, `SHELL`, `TMPDIR`, `TMP`, `TEMP`, `LANG`, `LC_ALL`, `PWD`) are no secret and stay in the server's `env`
     table. A name codex itself reads (`OPENAI_*`, `CODEX_*`, `SSL_*`, `NODE_*`, `LD_*`, `DYLD_*`, `RUST_*`, `*_PROXY`)
     isn't put in codex's environment (it would change codex's own account, endpoint or loader): that server goes
     without it. Two command servers asking for one name with different values: the first keeps it.
   - **opencode** already took its config in its environment (`OPENCODE_CONFIG_CONTENT`); **Grok** in a launch file;
     **Cursor** and **Antigravity** get no MCP servers from wisp.
2. **Launch files are this user's only and leave with the run.** The runtime writes them 0600 in a 0700 folder of the
   run's own under the OS temp folder (`wisp-run-<turn>`, `wisp-side-<id>`); a link at that name, or a folder another
   user made there (its mode can't be changed), is refused and nothing is written. A guard held by the run's drive
   (`LaunchDir`) removes the folder however the run ends: it finished, it never spawned (a spawn error, its folder gone,
   a cancel), or its drive was dropped (wisp.runtime switched off, wisp stopping). A crash leaves it: at start wisp
   removes such folders older than a week (a newer one may be another wisp's running run on this computer).
3. Header names and variable names may appear on argv (`x-wisp-token="WISP_MCP_TOKEN_0_0"`); values never do.

## Consequences

- The threat is another local user reading argv. The run's own agent runs as the user and can still read its parent's
  environment and the config file (its path is on argv), as it could read argv before: a token scoped to its own task
  (ADR-0112) is what limits it.

- `ps` during a run shows the config file's path and variable names, never the token or a connector's secret.
- A harness that takes a new kind of config says how it gets it in its launch, held by the test below.
- The fake agent records the config file's contents and the variables codex reads (`mcpConfig`, `mcpEnv`), so tests read
  the token there instead of argv.
- Reaches (ADR-0040): every place a task runs (the app, the CLI, the user's harness through MCP, channels, automations).

## Edge cases

| Case | What happens | Test |
|---|---|---|
| A connector with a secret env, a task on each of the 7 harnesses | no recorded argv holds the secret, the token or a token-valued `x-wisp-token`; Claude's and Copilot's config files are gone after the run | `crates/wisp/tests/serve_connectors.rs` `no_harness_gets_a_secret_or_its_wisp_token_on_its_command_line_audit_i_is3` |
| Claude's launch | `--mcp-config {tmp}/wisp-mcp.json`, the JSON in the file | `plugins/claude/host/src/launch.rs` `mcp_servers_go_in_a_file_never_on_argv_audit_i_is3` |
| Codex's launch, a `PATH` in a connector's env, two servers with one name | variables by name; `PATH` stays in `env`; the second server's value isn't passed; the user's shell policy untouched | `plugins/codex/core/src/argv.rs` `no_secret_goes_on_argv_audit_i_is3` |
| Copilot's launch | `@file`, the file holds v1's JSON | `plugins/copilot/core/tests/g6.rs` `every_fixture_builds_v1s_argv`; `wisps_http_server_goes_in_the_mcp_config` |
| The launch folder's and file's modes, a symlink at the folder's name | 0700 / 0600; refused | `plugins/runtime/host/src/run.rs` `launch_files_are_this_users_only_audit_i_is3` |
| A run that never spawned, or whose drive was dropped | its folder is removed | `plugins/runtime/host/src/run.rs` `a_runs_launch_folder_goes_however_the_run_ends_audit_i_is3` |
| A connector env named `OPENAI_API_KEY` for Codex | not put in codex's environment | `plugins/codex/core/src/argv.rs` `no_secret_goes_on_argv_audit_i_is3` |
