# ADR-0102: Every catalogue verb declares its arguments, a lazy plugin's too

- Status: accepted
- Date: 2026-11-18
- Source: port-grid's CLI safety fix (QA round 9 #1, `words::VERB_PATHS`, `tests/serve_cli_verbs.rs`): 34 catalogue verbs
  took any flag and exited 0. Follows ADR-0022 (the generated CLI) and ADR-0011 (lazy plugins).

## Context

The generated CLI checks a verb's flags against its argument type (`Command::args_ts`). A verb without one takes
each flag as a field (`--dry-run` → `dryRun`), as it must for an exec plugin's command. 167 catalogue verbs had none,
and 34 of them took nothing they needed, so `wisp stuff list --definitely-not-a-flag` ran and exited 0. Three things
were behind it:

1. **No declared type:** the command's `Args` was `Value` or a struct without `ts_rs::TS`.
2. **A lazy plugin:** a plugin that activates on its first call (`issues`, `onCommand:issues.list`) is listed from its
   manifest until then, with no type, even though its Rust command has one.
3. **Built-in commands with their own parsing:** `wisp mcp` (the stdio server) and `wisp plugin ls` don't go through
   the generated verbs, and they ignored flags.

## Decision

1. Each of those commands declares its type, as typed verbs do (`args_ts` from `ts_rs::TS`). The plugins' `command!`
   macros take an optional trailing `; typed` that emits it; commands written out by hand get it written out.
   - Defaulted fields are optional in the type (`#[ts(as = "Option<…>", optional)]`), so a verb still runs with no
     flags (`wisp git github-repos`, `wisp issues list --project …`).
   - Commands whose handler ignores its `Value` args keep `Value` on the wire, so a caller's `null` still works. They
     declare `JsonValue` (flags refused, `--args JSON` passes) or a documented type (`cli.catalogue`'s `CatalogueArgs`).
2. **`cli.catalogue { wake: <command id> }`** (additive; kernel `Ctx::catalogue_waking`): the catalogue with that
   command's lazy plugin woken first. It wakes only when the caller may run the command and its entry has no type
   yet, so the answer carries the type the plugin registers. The generated CLI asks it only for a verb listed without
   a type, the moment it runs that verb, which would wake the plugin anyway. Lazy activation stays lazy for
   everything else.
3. `wisp mcp` refuses any flag but `--repo <dir>` and the global ones; `wisp plugin ls` refuses any but the global
   ones (`words::stray_flag`, what `bad_flag` checks verb paths with).

## Edge cases (each with its test)

| Edge case | Test |
|---|---|
| Every built-in verb path and every catalogue verb refuses a flag it doesn't take, and none makes a task | `crates/wisp/tests/serve_cli_verbs.rs` `no_verb_path_with_a_bad_flag_ever_becomes_a_task` |
| The flags they do take still work (`stuff find --query --limit`, `resources export --kinds`, `resources write --format`, `mcp --repo`, `plugins list --json`, no flags at all) | the same test |
| A lazy plugin's verb knows its flags (`wisp issues list --help` lists `--project`) | the same test |
| `stray_flag` names the first flag a command doesn't take, skips a flag's value, stops at `--` | `crates/wisp/src/words_tests.rs` `a_command_that_reads_its_own_flags_names_the_first_it_doesnt_take` |

## Consequences

- 133 catalogue verbs are still untyped. Each refuses a bad flag today only because it needs a field the flag isn't,
  so its server error says what's missing, not its usage. Typing them is a later job, one plugin at a time.
- An untyped verb costs one more `cli.catalogue` round trip when it runs (to learn whether it's untyped for good: an
  exec plugin's command is).
