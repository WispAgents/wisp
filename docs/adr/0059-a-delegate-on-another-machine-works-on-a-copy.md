# ADR-0059: A delegate on another machine works on a copy under that wisp's own folder, and its changes come back as a patch

- Status: accepted
- Date: 2026-11-14
- Source: F-mach-7 (docs/v1-features.md: "Given another machine, when I delegate to it, then the delegate runs there
  and says where", partial), v1 `server/services/relay.js` (claim, launch, watch, return), `server/subagents.js`
  (`AWAY_LEAD`, `AWAY_SUBAGENT`), v1 #258, #260, #442; ADR-0010 (machines), ADR-0047 (sandboxes), ADR-0003 (plugins
  talk through their api).

## Context

`runtime.delegate` refused a team member whose `machine` is another computer ("can't take delegated work yet"): only
a sandbox (ADR-0047) or a crew member on that machine could take the work. v1 ran such a delegate on a copy of the
lead's folder there and applied its changes to the lead's folder when it finished.

`sandbox.seed` is the nearest thing, and it rightly refuses outside a sandbox: it writes `~/code/<name>`, which on a
real computer can be the user's own checkout. And the way a sandbox gets its project, `machines.route` with the bundle
inside the JSON, can't carry a real repo to a machine: the far side's `POST /api/events` takes 2 MB at most
(`wisp-api` `MAX_BODY`), so a bundle above about 1.4 MB never arrives. The wisp repo's own history is more than 30 MB.

v1's bugs here: a delegate's card said *This computer* for work that ran elsewhere (#442); the lead's absolute paths
were rewritten everywhere in the prompt, also in the middle of a longer path that only contains them (`/backup/repo/x`
lost its `/repo`); and a member's next delegation re-packed the lead's work, so a report never read lost the
delegate's changes.

## Decision

1. **`wisp.machines` owns the copy**, as four typed commands in `wisp-machines-api` (`carry.rs`); the runtime calls
   them through that api, never a host:
   - `machines.carry {id, cwd, key}` → `{cwd, machine}` (here): packs the lead's folder and sends it over the
     machine's own link; answers the copy's folder there and the machine's name.
   - `machines.takeIn {key, name, head, branch, author…}` (on the machine): unpacks it.
   - `machines.changes {cwd}` (on the machine, read-only): the copy's changes since it came in.
   - `machines.bring {id, from, into}` (here): those changes into the lead's folder.
   Each is default-deny: the runtime's manifest names `machines:carry` and `machines:bring`, and the machine's wisp
   answers `takeIn`, `changes` and the upload route only for a caller holding `machines:takeIn` / `machines:changes`
   (a grid box's set denies `machines:*`, so a sandbox keeps `sandbox.seed`).
2. **What goes.** The lead's committed history as `git bundle create <f> HEAD <branch>`, plus its uncommitted work
   (edits and new files; ignored files stay behind, as in v1) as a binary diff from `HEAD` to the folder's whole state,
   read through a scratch index so the lead's own index is never touched. Nothing is committed or pushed for it, so a
   dirty tree is not refused: a lead mid-task always has one, and v1 told leads "don't commit or push to hand it work".
   The machine needs no clone of the repo and no access to its remote. At most 256 MB of bundle (refused up front).
3. **How it goes.** The bundle and the patch are raw bytes to `POST /api/machines/carry/<key>/<part>` on the machine
   (the route's own cap: 257 MB), over the machine's channel (its transport, its token), then `machines.takeIn` over
   `/api/cmd`, with a 10-minute timeout: never inside a sync message.
4. **Where it lands.** Only `<wisp home>/plugins/wisp.machines/copies/<key>/<name>` on the machine, both parts cut to
   one path segment (`../../code` becomes `-..-code`). Never a folder of the user's there, so no checkout is ever
   overwritten. `key` is `<lead>-<member slot>`: one copy per lead and member, made afresh for each job. The lead's
   branch is checked out at the lead's commit (`wisp/delegate` when the lead is detached), the remote is removed
   (a delegate never pushes), and the lead's author is set.
5. **What comes back.** `machines.changes` answers only for a folder two segments below `copies/` with no `..`
   (never the user's checkout's diff). The copy's state when it came in is kept as a tree id in its `.git`, so the
   changes are the difference from that to its whole state now (edits and new files, nothing of the lead's own
   uncommitted work), with no commit made and no ref moved; at most 16 MB. `machines.bring` applies them to the lead's
   folder with `git apply`: uncommitted changes, nothing committed. A patch that clashes with what changed here
   meanwhile moves nothing; it's kept in `<wisp home>/plugins/wisp.machines/patches/<id>.patch` and the report says
   where (v1 kept it in `~/.wisp/patches`).
6. **The runtime.** A member whose `machine` is neither empty, `here` nor `sandbox` is placed there: `machines.carry`,
   then `runtime.create` routed to the machine (`machines.route`), cwd = the copy. The lead's absolute paths in the
   prompt become paths in the folder (`in_folder`, only where the lead's folder starts a path), and the delegate is
   told it's on another computer (`AWAY_SUBAGENT`). The next delegation to the same member: refused while its last job
   still runs; otherwise that job's changes come back first, then the copy is made afresh and the same task there is
   followed up (its conversation goes on). A capped lead's work never leaves this computer (refused, as a sandbox is).
   Reports are read there (`runtime.get`, `runtime.turns` routed); a done job's changes come back once per job (the
   lead keeps `<key>@<turn there>` it brought).
7. **"Says where."** The delegation's `place` is the machine's name; the lead keeps `away: {key: name}`; its turn's
   delegation record has `place`; its row reads *Delegated to far on studio*; every report from it ends *(It ran on
   studio, on a copy of your folder.)*, and says how many files came back or why they didn't.
8. **Reach (ADR-0040).** The delegate tool's MCP exposure is unchanged: `runtime.delegate` and `runtime.results` are
   the same commands. The four `machines.*` commands are in the catalogue like any other (an agent with `machines:*`
   reaches them; they write only under wisp's own folder or into a folder the caller names, as `runtime.delegate`
   already can).

## Consequences

- No new crate dependency, no new wire message: a new route and four commands in an existing plugin. The machines
  host's manifest gains `proc:exec{bin:git}`.
- A copy stays on the machine after its job (under wisp's folder, a fresh one each job); the task there stays too, so
  the user can open it from the machine's list. Deleting the lead doesn't delete them yet (v1 deleted the task there
  after each job; that is the next step, with the sweep ADR-0047 has for sandboxes).
- Git-ignored files (`.env`, `node_modules`, build output) and anything outside the folder don't go, as in v1; the
  lead hears so (`AWAY_LEAD`). A lead folder that isn't a git repo with a commit can't delegate to a machine.
- The whole history goes each job. An incremental bundle (`<what the machine has>..HEAD`) is an optimisation for later.

## Edge cases

Each one is a test named after it.
- `a_copy_lands_under_wisps_own_folder_never_the_users_checkout` (machines host)
- `a_copys_changes_are_the_difference_from_what_came_in` (machines host)
- `only_a_carried_copy_gives_its_changes_back` (machines host and core)
- `a_lead_folder_that_isnt_a_repo_or_an_unknown_machine_is_refused_before_anything_goes` (machines host)
- `carrying_and_taking_in_need_their_permissions` (machines host)
- `a_bring_that_clashes_keeps_the_patch_and_moves_nothing` (machines host, unit)
- `a_copy_is_always_under_wisps_own_folder`, `a_detached_lead_gets_a_branch_of_its_own_there` (machines core)
- `a_delegate_elsewhere_reads_the_leads_paths_as_paths_in_its_folder` (runtime core)
- `a_delegate_on_another_machine_runs_on_a_copy_there_and_says_where` (runtime host: once per job, the next job
  re-carries and follows up)
- `a_capped_leads_work_never_leaves_this_computer`, `a_machine_that_refuses_the_copy_is_that_tasks_error` (runtime host)
- `a_delegate_on_another_machine_runs_there_says_where_and_its_change_comes_back` (crates/wisp, two `wisp serve`)
