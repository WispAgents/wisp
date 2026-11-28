# ADR-0148: A team's hub is a wisp on the grid that the team owns, deployed from a folder like Terraform

- Status: accepted
- Date: 2026-11-28
- Source: the owner: "make the grid a central place for teams to run common agents. `wisp grid init` creates a folder
  for agents, triggers etc. that can be deployed to the grid (think terraform), from the CLI and from GitHub Actions
  on merge; in a grid users generate keys to use in the deployments." Then: projects as part of it, a GitHub App the
  team installs, creating the hub from the grid UI or the CLI with members, admins and keys, tasks sandboxed inside the
  hub, the hub opening as a UI like the grid, and the hub as a machine in the desktop app.
  Builds on ADR-0014 (the grid's trust table), 0026/0034 (wisp as code, folders), 0041 (teams), 0043 (triggers),
  0047 (sandboxes) and 0058 (API keys).

## Context

- **Teams exist but run nothing.** `apps/grid/src/teams.rs` holds a team and its members with a role
  (owner, admin, member, viewer). Shares can be for a team. A team has no box, so the agents its people share still
  run on each person's own wisp.
- **Deploying exists but needs your own wisp.** `wisp grid deploy -f` runs `code::apply` through the local wisp and
  `machines.route` to *your* box, signed in with your device token. A CI runner has neither.
- **A box is per user.** hostd starts a box for a `users` row (`BoxUser`). The edge opens `<handle>.<domain>` for that
  row's own sessions, devices, cookies and `wgk_` keys, and refuses everyone else (`edge_reach.rs`).
- **Sign-ins and env for boxes are sealed per user** (`sandboxes.rs` `set_env`, `set_login`, `env_for`). Today only
  sandboxes get them.
- `wgd_` is already the device token's prefix, so a deploy key needs its own.

## Decision

### 1. A hub is a team's own account on the grid

- Making a team also makes its **hub**: a `users` row whose `handle` is the team's slug, with no identity, so nobody
  can sign in *as* it. `teams.hub_user_id` points at it (a new grid.db migration). A team slug and a handle share one namespace,
  so a slug someone already holds as a handle is refused ("`acme` is taken"). A team made before this gets its hub the
  first time it's asked for (`wisp team hub acme`), as long as its slug is free.
- The hub's box is an ordinary box: its address `<slug>.<domain>`, sleeping and waking, boxd, the waiting page, and the
  wisp UI. **The hub opens as a wisp in the browser**, as a person's box does.
- `/auth/dev` refuses a hub's handle, and the account and seat lists skip hubs.
- Deleting a team (owner only) deletes its hub: its box and home, its keys, invites, deploys and secrets.

### 2. Who gets in, and as what

The edge opens a hub's address for **the team's members**, by their role. It tells the box's wisp who is asking with
`x-wisp-proxy-via` (dropped from callers, set only by the edge), and the box's wisp gives a capability set:

| asker | via | set on the hub's wisp | may |
|---|---|---|---|
| an owner or admin (session, device, cookie) | `owner` | `hub:owner` | everything a box's person may, and the hub's machines: a hub's only machines are the sandboxes it makes (a box never syncs a laptop), so `box`'s `machines:*` deny would stop its people starting sandboxed work |
| a member | `member` | `hub:member` | start and follow up tasks (sandboxed too), run triggers and workflows, read everything; no settings, secrets, plugins, accounts or resources |
| a viewer | `viewer` | `hub:viewer` | read topics, sessions, stuff |
| a deploy key (`wgt_…`, rights deploy) | `deploy` | `hub:deploy` | plan and apply: what each kind reads and applies through its own plugin's commands (crew, triggers, projects…), but no work (no task, follow-up, wake, fire or run), no session, no secret, no machine, no key |
| a plan key (`wgt_…`, rights plan) | `plan` | `hub:plan` | `hub:deploy`'s set, read-only |
| anyone else | — | — | 403 "That hub is acme's: ask an owner to add you." |

The trust table gets one credential, `HubKey`: it reaches `Box` (its own hub only, checked like a `wgk_` key's
handle) and nothing else; the deploy history is `GET /_grid/deploys` at the hub's own address. A `wgk_` key never opens
a hub. A read-only set (`readOnly` in `capabilities.json`) is new in the kernel: a command that writes can't start
under it (`dispatch.rs`), whatever its allow list says; viewers and plan keys hold one.

### 3. People: create, invite, roles

- **Create:** `wisp team create <slug> [name]` or the grid's **New team**. The creator is the owner.
- **Add by handle** (exists): `wisp team add acme bo admin`.
- **Invite links:** `wisp team invite acme --role member [--expires 7d] [--uses 10]` makes a link
  `<grid>/join/<code>`. The code is shown once and kept as its SHA-256. Opening it signed in adds you with that role.
  An invite can't give `owner`, an admin's can't give `admin` or above, and it's refused when spent, expired or
  revoked. At most 20 open invites a team.
- **Roles:** `wisp team role acme bo admin`, `wisp team rm acme bo`. The existing rules hold: a team keeps an owner,
  and an admin manages members and viewers but not owners.

### 4. Deploy keys

- An owner or admin makes one: `wisp team key create acme --name ci-main [--rights plan|deploy]` or the team's Keys tab.
  `deploy` (the default) can plan and apply; `plan` can only plan, for pull requests.
- Shown once; kept as its SHA-256 and last four characters; `last_used_at` written at most once a minute; revoked at
  once; at most 20 live keys a team; `audit` gets `hubkey.create` / `hubkey.revoke` with the key's id only.
- Creating a key is never an agent's: through MCP a key would sit in a transcript (ADR-0058's rule). Listing and
  revoking are.

### 5. Deploying: the folder, the plan, the history

- `wisp grid init <team> [dir]` writes a formula folder (ADR-0034): `grid.yml` (the grid, the team and the hub's
  address), `projects/`, `agents/`, `triggers/`, `README.md`, and `.github/workflows/wisp-hub.yml` at the repo's root.
  It never overwrites a file.
- `wisp grid plan` / `wisp grid deploy [--yes] [--prune]` evaluate the folder on the caller's machine and run the same
  `resources.plan` / `resources.apply` as `wisp apply`, sent straight to the hub's address with
  `Authorization: Bearer <key>` (`WISP_HUB_KEY`): **no local wisp is needed**, so it works in a bare CI runner. Without
  a key, through the person's own wisp and its grid sign-in (`grid-client.hubCall`, `resources.*` only).
- The edge holds **one deploy at a time per hub**: a second `resources.apply` while one runs is refused with 409
  "deploy #14 by ci-main is running". A deploy that dies gives up its lock after 10 minutes.
- Every apply that changed something is recorded (`team_deploys`): its number, who (person or key), its source
  (`x-wisp-deploy-source`: repo and commit), its counts (`+2 ~1 -0`), when, and how it ended.
  `wisp grid deploys` lists them, and so does the team page.
- `--prune` deletes only what earlier deploys made (the resources engine's claims, AAC-1), never what someone made by
  hand on the hub.

### 6. Secrets and harness sign-ins

- `wisp team secret set acme ANTHROPIC_API_KEY` (from stdin) or the team's Secrets tab seals the value per hub on the
  grid (the sandboxes' sealed env, keyed by the hub's account). `wisp team secret ls` lists names only.
- The hub's box starts with them in its environment, and so does every sandbox the hub owns. A formula refers to them
  as `${env:NAME}` or `${secret:NAME}`. They are never in the repo.
- A running hub box gets a new value when it next starts: `wisp team restart acme` (or the hub's next sleep and wake).

### 7. Projects

The `project` kind (ADR-0026) gains `repo` (`https://github.com/acme/api`, `git@…` or `acme/api`). A project with a
`repo` and no `path` is cloned on apply (`git.clone`, `gh repo clone` first, so a hub's `GH_TOKEN` reaches a private
repo) into `<wisp home>/repos/<project>/<repo>`, which becomes its path; a formula that names only the repo plans as
unchanged once it's cloned (`normalize_named`). Triggers and crew already point at projects by name. A `ref`, and a
fetch before each task, are later.

### 8. Tasks in sandboxes

A hub's box runs `HubPlugins` (the box group with `machines`): the grid tells it it's a hub (`WISP_HUB=1`), so its
sandbox plugin can keep the sandboxes it makes as peers.

The hub's own box can make sandboxes for itself (its `Box <handle> <secret>` credential, which the grid takes from a
hub's box only, counted against the hub's sandbox cap), so `/sandbox`, `sandbox.place` and crew members'
`sandbox: job` work on a hub as on a laptop. A sandbox the hub owns gets the hub's secrets. `wisp grid init`'s
agent sets `sandbox: job`; a default of a sandbox for every hub task, and fork PRs always sandboxed, come later (§10).

### 8b. Webhook triggers from the internet (with ADR-0147's tokens)

- A hub's webhook triggers get URLs at the hub's own address: `https://<team>.<domain>/_hook/wgh_…`, ADR-0147's token
  shape, kept in `team_hooks` (the team's, as a computer's are in `hook_tokens`): said once, its SHA-256 and last four
  characters kept, at most 50 live per hub, revoked at once, gone with the team.
- The call (`edge_hub_hook.rs`), before any sign-in: a token unknown, revoked or another hub's is the same 404; over
  1 MB, 413. It goes to the hub's wisp as `POST /api/triggers/<trigger>` with the caller's own headers and none of the
  grid's (ADR-0147's `hook_headers`, which now drops a `wgt_` bearer too), marked `x-wisp-proxy-via: hook`, so the hub
  takes it as `public` and the trigger checks its own secret or signature. The box wakes for it: nothing is held.
- Who makes one: an owner or admin (the team's page, `wisp team url`, `grid-client.hubHookCreate`, uiOnly), or the hub's
  own wisp for its Automations page: `grid-client.hooks*` on a hub's box (`WISP_HUB=1`) asks `/api/hooks` with the box's
  secret, which the grid takes from a hub's box only. ADR-0147's rule stands: a person's box makes no URL (a hub's URL
  starts work on the hub, never on a laptop).
- **Team secrets as `${secret:NAME}`:** the grid gives a hub's box the names of its team's secrets
  (`WISP_HUB_SECRETS`); wisp's secrets service answers those names from its environment when `secrets.json` has none
  (the file's own value wins). So a trigger's `secret: ${secret:SENTRY_HOOK}` and plan's missing-secret check both see
  a secret set with `wisp team secret`.

### 8c. A hub works while nobody has it open (found running it end to end)

- **It wakes for its triggers.** wisp.automations reports the earliest moment a live trigger needs the tick (its next
  poll, a schedule's next minute, a one-off, a batch closing) as `nextWakeAt` in `GET /api/status` (`kernel.status`),
  so hostd wakes a sleeping box then. No plugin reported it before: a cloud box slept through every poll and schedule.
- **A lazy source is woken for its trigger.** A plugin that declares `contributes.triggerSources` activates on
  `onTriggerSource:<on>` (implicit, as a declared command is `onCommand`), and the engine asks for that (`Ctx::wake`,
  additive) when a poll finds no active source. `wisp.issues` activated only on `issues.list` before.
- **A polled event comes from its trigger's project:** the engine sets its `cwd` to the trigger's project when it names
  none, so `project:` (a filter on these events) passes; issue and PR events never carried one and were all dropped.
- **Any harness in a sandbox**, as wisp.sandbox's `may_start` already said: a crew member with `sandbox: job` was refused
  anything but Claude. `wisp grid init`'s triager runs on opencode's free model, which needs no sign-in.
- **A queued crew job is `queued`** in a trigger's log, never a failure.
- **Its sandboxes sleep.** Only a write is use: wisp's edge counts a `POST /api/cmd` or a sync `mutate` only for a
  command that isn't read-only (a link's hello, sub or window never). The grid leaves use on those two routes to the
  box's own `lastActivityAt`, never holds a box up under a passive event stream (someone watching), and doesn't touch a
  running box for a passive request. The grid transport connects with a passive look first (it wakes only an asleep
  box), wisp.sandbox's owner check waits one tick after start (the saved state says "online"), and crew counts live
  jobs without asking an asleep sandbox (a live turn keeps a box up). Each of these kept a job's sandbox up for good.
- **A next wake survives a stop at start:** a box says no `nextWakeAt` until its plugins load; for its first minute the
  one it had (still ahead) stands in, so a grid restart then doesn't lose a hub's schedule.

### 8d. A hub is called by its name

`PeerInfo` (the sync welcome) gains an optional `kind`, `"hub"` on a team's hub (`WISP_HUB=1`), and the grid starts a
hub's box with `WISP_NAME="<Team> hub"`, so the welcome names it. Where a lazily loaded view says "This computer" (the
task's side panel, crew's places, Automations' words) a hub says its name and a server icon (`@wisp/shell`'s
`useHere`). The first screen's own labels (New task's place, the list's computer, the Projects sheet) keep "This
computer": the phone's first-screen JS sits at its budget (ADR-0011), and naming the hub there cost ~200 B br. An older
client ignores `kind`; an older server sends none, which reads as a computer.

### 9. The desktop app and the GitHub Action

- **A hub is a machine.** `grid-client` lists the hubs of the signed-in person's teams (`/api/me` gains `hubs`) and
  adds each as a `grid` machine, so the desktop app's list, `--on acme` and the TUI reach it like a cloud box.
- **The Action** (`action/`) gains `deploy: plan | apply` with `hub-key`: `plan` on pull requests (the plan posted as
  a comment), `apply` on a push to main.

### 10. Later, not here

- `ci.*` from the App's `check_suite`/`workflow_run` webhooks (they poll for now), a sandbox's token renewed past its
  first hour, and a default of a sandbox for every hub task with fork PRs always sandboxed.
- Several spaces per team (staging, prod), GitHub OIDC instead of a stored key, members declared in the folder.

## Consequences

- A hub costs one box, which sleeps like any other, and one row in `users`.
- The edge's box path asks one more indexed question per request (is this handle a hub).
- Three capability sets join `capabilities.json`, and one credential and one reach join the grid's trust table
  (`tests/trust.rs` pins them).
- Places it reaches (ADR-0040): the grid's pages, the CLI (`wisp team`, `wisp grid`), MCP (`grid-client.teams*`, but
  not key creation), the GitHub Action, the desktop app's machines.

## Tests

| What | Where |
|---|---|
| team create makes the hub; a taken slug is refused; dev sign-in refuses a hub | `apps/grid/src/teams.rs` tests |
| members in by role with the right via; outsiders 403; a hub key opens its own hub only | `apps/grid/tests/hubs.rs` |
| invite codes: once, roles capped by the inviter, spent/expired/revoked refused | `apps/grid/src/hubs.rs` tests |
| keys: shown once, rights, 20 cap, revoke, `wgk_` never opens a hub | `apps/grid/tests/hubs.rs` |
| one deploy at a time; history rows with source and counts | `apps/grid/tests/hubs.rs` |
| a member's set can't change settings or secrets; a deploy key only resources | `crates/wisp-api/src/auth.rs` tests |
| `wisp grid init` writes the folder and never overwrites | `crates/wisp/src/grid_init.rs` tests |
| a hub's trigger URL: said once, as no one with the caller's headers, other hub/token/name 404, the hub's box manages its own, a person's box can't | `apps/grid/tests/hubs.rs`, `apps/grid/src/hub_hooks.rs` |
| team secrets answer as `${secret:NAME}` on a hub only, the file wins | `crates/wisp/src/services_tests.rs` |
| a trigger's next wake (poll, schedule, one-off, batch); a polled event passes its project filter; queued words | `plugins/automations/host/src/engine.rs` `wake_tests` |
| a declared trigger source wakes its lazy plugin | `crates/wisp-kernel/src/activation.rs` |
| a sandboxed crew member runs any harness | `plugins/crew/core/tests/sandboxed_harness.rs` |
