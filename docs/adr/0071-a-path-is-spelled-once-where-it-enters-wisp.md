# ADR-0071: A path is spelled once, where it enters wisp

- Status: accepted
- Date: 2026-11-15
- Source: round 263, CI 8bb8ccd0 (windows-latest, node 22): 14 serve tests. Builds on round 257 (`long_path`,
  commits 7ccc17f3, d90e5992).

## Context

Windows names one folder two ways: its long name and an 8.3 short name. The runner's `%TEMP%` is
`C:\Users\RUNNER~1\AppData\Local\Temp`, and the same folder read back through `canonicalize`, a worktree under the home or
`GetLongPathNameW` is `C:\Users\runneradmin\...`. Round 257 spelled a task's folder and a project's path long. Every other
path that comes in (the wisp home, `$HOME`, a trigger's project, a `projectRun` cwd, a copy's cwd) stayed as it was given.
A comparison between a stored long path and a short one given later then failed inside wisp: `runtime.projectRun` said
"isn't in a declared project", a project's worktree and setup never ran (the project was matched before the folder was
spelled), a trigger on the project never fired, a delegate's changes on another machine were refused (HTTP 403 "not a
copy": the plugin's folder was short, the copy's cwd long), and a sandbox refused "only a project under ~/code".

## Decision

1. **One helper.** `wisp_proc::plain_path` (and `_blocking`, `plain_str`) spells a path the one way wisp keeps it: 8.3
   names spelled out, no `\\?\` prefix, links not followed. When the path isn't there yet, its longest part that exists is
   spelled out and the rest is kept. On Unix it only removes the verbatim prefix and makes no file-system call. The rule
   itself is `wisp_proc::spell_out_with`: pure, on text, with either separator, and given a mapping, so it is tested on
   every OS. `wisp_proc::within_with` / `within_blocking` spell both sides first and then compare by parts.
2. **Spelled where it enters**, once. The wisp home and the user's home (`services::home`, `services::user_home`), the
   CLI's folder (`main`), `runtime.create`'s `cwd` and `dirs` before anything reads them (`specs::spell_folders`, ahead of
   the project match), a project's `path` on apply (`tidy_project`), a trigger's `project` and `match.project` on apply
   (`TriggerKind::normalize`), `runtime.projectRun`'s cwd, `machines.changes`' cwd, the sandbox's home and
   `sandbox.patch`'s cwd.
3. **Comparisons compare spelled values.** Each check that uses `starts_with`, `strip_prefix`, `==`, `is_within` or
   `same_folder` on paths has both sides from (2), or from what wisp derived from them (copies, worktrees and scratch
   folders under the home). A trigger's folder filter matches a Windows path (a drive or a UNC share) by parts too, with
   either separator and in any case. Before, only `/...` paths were treated as folders, and `C:\...` needed an exact match.
4. **Tests spell their folders the same way.** `crates/wisp/tests/common::temp` and runtime's `tests/common::tmp` return
   the long spelling, so an assertion compares what wisp stores.

### Edge cases (each with its test)

- A short spelling is spelled out, and so is a path that isn't there yet (only the part that exists), the verbatim
  prefix goes, and a relative path or a root is left alone:
  `crates/wisp-proc/tests/canonical.rs` `an_8_3_name_is_spelled_out_by_the_longest_part_that_exists`.
- A subfolder in short spelling is within its project's long spelling. A sibling with a shared prefix is not, and
  neither is a parent: `canonical.rs` `a_short_spelling_is_within_its_long_one`.
- On the real file system, a temp folder reads as its canonical path on Windows, and every path is unchanged on Unix:
  `canonical.rs` `a_short_name_is_spelled_out_and_nothing_else_changes`.
- A trigger on a Windows project hears its tasks and their subfolders: `plugins/automations/core/src/engine_tests.rs`
  `a_trigger_on_a_windows_project_hears_its_tasks_and_their_subfolders`.

## Consequences

- Windows makes one `GetLongPathNameW` call per missing part where a path enters. Unix makes none.
- Case is not changed. A comparison that has to ignore case (NTFS) still says so (`folder_key`, `within_with`).
- A path a plugin reads from somewhere else (a harness's session log, `git rev-parse`) is that source's spelling. It is
  compared through `paths::same_folder`, as before. When one of these turns out short, it gets spelled where it is read.
