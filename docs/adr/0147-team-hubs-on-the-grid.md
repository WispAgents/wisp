# ADR-0147: A team's hub is a wisp on the grid that the team owns, deployed from a folder like Terraform

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
| an owner or admin (session, device, cookie) | — | `box` (as a person's own box) | everything a box's person may |
| a member | `member` | `hub:member` | start and follow up tasks, run triggers and workflows, read everything; no settings, secrets, plugins, accounts or resources |
| a viewer | `viewer` | `hub:viewer` | read topics, sessions, stuff |
| a deploy key (`wgh_…`) | `deploy` | `hub:deploy` | `resources:*` (kinds, plan, apply, export, refresh), `topic:read`, `edge:health`; nothing else |
| anyone else | — | — | 403 "That hub is acme's: ask an owner to add you." |

The trust table gets one credential, `HubKey`: it reaches `Box` (its own hub only, checked like a `wgk_` key's
handle) and `GridHub` (the hub's deploy history), and nothing else. A `wgk_` key never opens a hub.

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

- `wisp grid init [dir] --team acme` writes a formula folder (ADR-0034): `grid.yml` (the grid's URL and the team),
  `projects/`, `agents/`, `triggers/`, `workflows/` (empty), `README.md`, and `.github/workflows/wisp-hub.yml`.
  It never overwrites a file.
- `wisp grid plan` / `wisp grid deploy [--yes] [--prune]` evaluate the folder on the caller's machine and run the same
  `resources.plan` / `resources.apply` as `wisp apply`, sent straight to the hub's address with
  `Authorization: Bearer <key>` (`WISP_HUB_KEY`) or the person's device token. **No local wisp is needed**, so it works
  in a bare CI runner.
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
- Setting a secret restarts a running hub box once it's idle, so the next task sees the new value.

### 7. Projects

The `project` kind (ADR-0026) gains `repo` (`github.com/acme/api`) and `ref`. On a hub, a project with a `repo` and no
`path` is cloned into the hub's home on apply and fetched before each task, with the hub's `GH_TOKEN`. Triggers and
crew already point at projects by name.

### 8. Tasks in sandboxes

The hub's own box can make sandboxes for itself (`Box` credential, a hub's box only, counted against the hub's
sandbox cap), so `/sandbox`, `sandbox.place` and crew members' `sandbox: true` work on a hub as on a laptop. A
sandbox the hub owns gets the hub's secrets.

### 9. The desktop app and the GitHub Action

- **A hub is a machine.** `grid-client` lists the hubs of the signed-in person's teams (`/api/me` gains `hubs`) and
  adds each as a `grid` machine, so the desktop app's list, `--on acme` and the TUI reach it like a cloud box.
- **The Action** (`action/`) gains `deploy: plan | apply` with `hub-key`: `plan` on pull requests (the plan posted as
  a comment), `apply` on a push to main.

### 10. Later, not here

- A GitHub App the team installs (short-lived per-repo tokens, webhooks feeding `pr.*`/`issue.*` triggers at once),
  with `GH_TOKEN` as a hub secret meanwhile and for self-hosted grids. Until then, `issue.*` and `pr.*` triggers poll
  through `gh` as they do on a laptop.
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
| `wisp grid init` writes the folder and never overwrites | `crates/wisp/src/hub_cli.rs` tests |
