# ADR-0060: A sandbox gets its project as raw bytes (`machines.send`), and a delegate's copy and task elsewhere go with its lead

- Status: accepted
- Date: 2026-11-14
- Source: round 183's open bug (a repo whose bundle is over ~1.4 MB never reaches a sandbox), ADR-0047 §3 (what goes
  into a sandbox) and §5 (deleted with its owner), ADR-0059 §3 (the raw carry route) and its *Consequences* ("Deleting
  the lead doesn't delete them yet"). Partly supersedes ADR-0047 §3: the project no longer travels inside
  `sandbox.seed`'s arguments.

## Context

`sandbox.place` and `sandbox.launch` packed the project (`git bundle --all`, base64) into `sandbox.seed`'s arguments
and sent them through `machines.route`. A routed command travels as one sync message, and the sandbox's wisp takes a
`POST /api/events` body of 2 MB at most (`wisp-api` `MAX_BODY`), so any repo whose bundle is over about 1.4 MB
(base64 grows it by a third) never reached its sandbox: the launch failed with the edge's 413. Most real repos are
over that; the wisp repo's history is more than 30 MB.

ADR-0059 already sends a delegate's copy raw (`POST /api/machines/carry/<key>/<part>`, 257 MB), but a sandbox can't
take it there: a grid box's capability set denies `machines:*`, so the box refuses `machines:takeIn` and its route.

And ADR-0059 left a delegate's copy and its task on the other machine behind when the lead was deleted.

## Decision

1. **`machines.send {id, cwd, key, to, all}`** (here, in `wisp-machines-api`'s `carry.rs`) is carry's pack and
   upload on their own: the folder packed (a bundle of HEAD and its branch, or of every ref with `all`, plus the
   uncommitted changes, new files too, read through a scratch index), each part `POST`ed raw over the machine's own
   link to `/api<to>/<key>/bundle` and `/api<to>/<key>/patch`. It answers what the far side needs to take it in
   (`head`, `branch`, `origin`, author). `to` is a route prefix of plain segments (never `/cmd` or `/events`).
   `machines.carry` is `send` (`to: /machines/carry`) then `machines.takeIn`. At most 256 MB of bundle
   (`wisp_machines_api::MAX_CARRY`), refused before anything goes: *The project is too big to send (N MB; at most
   256 MB).*
2. **A sandbox takes uploads itself:** `POST /api/sandbox/seed/<key>/<part>` (permission `sandbox:seed`, which a box
   holds; body cap `MAX_CARRY` + 1 MB), only inside a sandbox, written to `~/.sandbox/incoming/<key>.<part>`.
   `sandbox.seed {name, key, branch, origin, author…}` reads the upload by `key`; its own rules stay: it answers only
   inside a sandbox and writes only `~/code/<name>`. `wisp.sandbox` gains `machines:send`. A seed's base (what
   `sandbox.patch` diffs from) is the tree of what came in, new files too, so the lead's new files never come back
   as the delegate's.
3. **A delegate's copy and task go with the lead.** `runtime.delete` of a lead calls `machines.uncarry {id, lead,
   task}` for each of its delegations on another machine (`away`). Here, machines deletes the task there
   (`runtime.delete`, routed; already gone is fine) and its copies (`machines.dropCopies {lead}` there: every
   `copies/<lead>-*` folder and upload of that lead, only under wisp's own folder). A machine that can't be reached
   keeps the drop in its row (`data.uncarry`); a sweep tries again when the machine comes online and every 10 minutes,
   as ADR-0047's does for sandboxes.

## Consequences

- No new dependency, no new wire message: three commands and a route in existing plugins. `wisp-machines-host` now
  uses `wisp-runtime-api` (the far task's `runtime.delete`, typed).
- `sandbox.seed`'s inline base64 `bundle` stays for a seed with no upload (tests, a small project); the lead never
  sends one.
- The far wisp's own `MAX_CARRY` bounds a sandbox's project, the same as a delegate's on another machine.

## Edge cases

Each one is a test named after it.
- `a_project_bigger_than_a_sync_message_reaches_its_sandbox` (crates/wisp, a local grid: 3 MB of history and a 1 MB
  file not committed)
- `only_a_sandbox_takes_an_upload_and_a_seed_reads_it_by_key` (sandbox host)
- `a_send_goes_only_to_a_route_prefix` (machines core)
- `deleting_the_lead_lets_its_delegates_elsewhere_go` (runtime host)
- `a_machine_that_cant_be_reached_keeps_the_drop_for_the_sweep` (machines host)
- `dropping_a_leads_copies_takes_only_that_leads` (machines core)
- `deleting_the_lead_deletes_its_delegates_copy_and_task_there` (crates/wisp, two `wisp serve`)
