# ADR-0145: MCP Events both ways, poll first: an `mcp.event` trigger source and wisp's log on `events/poll`

- Status: accepted
- Date: 2026-11-27
- Source: the owner asked what MCP events are and how wisp can use them, then said "do 1 and 2": (1) wisp as an
  Events client (any MCP server's events start a task or wake a crew member) and (2) wisp as an Events server (any MCP
  client reacts to wisp's durable events). The extension is SEP-3415, `io.modelcontextprotocol/events`, merged as
  Final on 2026-11-19. Builds on ADR-0043 (one trigger engine, `trigger.source@1`), ADR-0044 (the durable log and
  its catalogue) and ADR-0040 (meet users where they work).

## Context

- **SEP-3415** lets an MCP client subscribe to things that happen upstream. A server lists event types
  (`events/list`: `name`, `inputSchema`, `payloadSchema`, `delivery`) and delivers them in one of three modes, none
  mandatory: **poll** (`events/poll {name, arguments, cursor}` → `{events, cursor, hasMore, truncated, nextPollMs}`),
  **push** (a long-lived `events/stream`) or **webhook** (`events/subscribe`, signed per Standard Webhooks, with a
  TTL). Cursors are opaque, a `null` cursor means "from now", and each `EventOccurrence` has a stable `eventId` for
  dedupe. It needs the `2026-07-28` protocol: the capability sits under `capabilities.extensions`, which
  `server/discover` returns, and a modern request carries its version in `_meta`.
- **Clients so far:** ChatGPT (webhook), plus experimental support in the Vercel AI SDK, Mastra, TanStack AI and
  Hermes. Claude Code doesn't support it yet, and no official SDK has a reference implementation.
- **wisp has almost all of it already:**
  - the trigger engine polls a `trigger.source@1` with a cursor it keeps in the trigger's row;
  - the durable log has seq cursors, retention and a catalogue of types;
  - capped agents are already scoped to their own task's events (`events.tail`).
- **What it lacked:**
  - wisp's MCP code spoke only `2025-06-18`.
  - The MCP server answered a fixed method list.
  - Connectors could call `tools/call` only.

## Decision

### 1. `mcp.method@1`: a plugin serves a JSON-RPC method on wisp's MCP endpoint

- **A new point in `wisp-mcp-api`:** a `MethodContribution` has a `method`, an optional
  `extension: (identifier, settings)` and a `run(ctx, call, params) -> Result<Value, MethodError>`.
  - `MethodError` carries `code`, `message` and a typed `data`, the way SEP-3415's codes do.
  - The call runs as the caller, so a task's agent's capability set and scoping apply.
  - A contribution can't take a core method (`initialize`, `ping`, `server/discover`, `tools/*`,
    `notifications/*`). The first plugin to name a method has it.
- **The extensions** contributed methods declare are listed under `capabilities.extensions` in `initialize` (for
  legacy clients that know the extension) and in `server/discover`.
- **Why a point, not code in wisp.mcp:** built-ins get no private API. The events plugin owns the log, so it serves
  the log's methods, and a user's plugin can serve another extension the same way.

### 2. Dual-era MCP server

- **`server/discover`** is answered (the `2026-07-28` spec says every server must). It returns
  `supportedVersions: ["2026-07-28", "2025-11-25", "2025-06-18", "2025-03-26"]`, the capabilities and
  `_meta["io.modelcontextprotocol/serverInfo"]`, with `cacheScope: "private"`, because an agent is offered
  different tools than the user.
- **A request whose `_meta` names a version** is modern: every result in its reply gets
  `"resultType":"complete"`.
- **A modern request naming an unknown version** gets `UnsupportedProtocolVersionError` (`-32022`) with
  `{supported, requested}`.
- **Every request is served statelessly**, as before. `initialize` without contributions is unchanged
  (`tests/goldens/initialize-user.json`).

### 3. wisp as an Events server (`wisp.events`)

- **`events/list`** lists the catalogue's **durable** types only, each with `delivery: ["poll"]`. Ephemeral
  events never reach the log, so a poll couldn't serve them.
  - Each type takes one argument, `subject` (a task id).
  - Its `payloadSchema` is `{type: object}`: the catalogue has no schemas yet.
- **`events/poll`** is a new read command, **`events.poll`**. It is also an MCP tool (`events_poll`) and a CLI verb,
  like every command.
  - **The cursor** is the log's seq, as a string. `null` returns no events and the newest seq ("from now").
  - **Retention:** a cursor older than what the log still keeps answers `truncated: true` and goes on from the
    oldest kept event.
  - **Batches:** a poll returns 50 events by default and 500 at most (`maxEvents`). It reads 20 000 rows at most
    before answering `hasMore`.
  - **Other parameters:** `maxAgeMs` is honoured, and `nextPollMs` is 5 000.
  - **Each occurrence:** `eventId` is the seq and `timestamp` is ISO 8601. `data` is the event's body, and
    `_meta["dev.trywisp/seq"|"dev.trywisp/subject"]` carries wisp's own fields.
- **Scoping and errors:** a task's agent may run `events.ls` and `events.poll` (`agent:session` gets `events:ls` and
  `events:poll` beside `events:tail`), and it polls its own task's events only. Another subject is `Forbidden`
  (`-32024`), an unknown or ephemeral type is `NotFound` (`-32023`), and a bad cursor is `InvalidParams`.
- **Not yet:**
  - `events/stream` and `events/subscribe` answer `Unsupported` (`-32026`, `data: {feature: "deliveryMode",
    value}`).
  - `events/unsubscribe` answers `NotFound` (`data.kind: "subscription"`).
  - Push already exists as the `events:<glob>` topic, and webhook as `events.sink@1`. Moving them onto the SEP's
    methods is later work (below).

### 4. wisp as an Events client (`wisp.connectors`)

- **`connectors.events {id}`** lists a remote connector's event types: every `events/list` page, ten at most.
  A server that answers `-32601` returns `{served: false}`. It is also the MCP tool `connectors_events`.
- **The `mcp.event` trigger source** is a `trigger.source@1` in poll mode, every minute by default. Its fields go in
  `match`, the source-fields convention of ADR-0043:

  ```yaml
  on: mcp.event
  match: { connector: pagerduty, event: incident.created, arguments: { severity: P1 } }
  prompt: "Triage this: {event.summary}"
  ```

  - **The first poll** sends `cursor: null`, so nothing from before the trigger fires.
  - **The kept cursor** is `{"c": <the server's cursor>}`, so a server that keeps no replay (`c: null`) is told
    `null` again rather than being treated as a first poll.
  - **`hasMore`** is drained at once, ten pages at most. `truncated` is logged.
  - **Each event's key** is `<connector>:<event>:<eventId>`, so the engine fires once per upstream event.
  - **The payload** echoes `connector`, `event` and `arguments`, so the engine's `match` filter passes them. Other
    `match` keys (`data.service: api`) filter on the server's data.
  - **The summary** is `connector: event: <data.title|subject|summary|name>`, one line, 120 characters at most.
  - **Event data counts as untrusted**, the same as tool results. It reaches a prompt only through the placeholders
    the trigger's author wrote.
- **Every request** goes through `call::request`, the same path as `tools/call`: the connector's headers, its
  sign-in and its token refresh. It carries the extension in
  `_meta["io.modelcontextprotocol/clientCapabilities"]`.
- **A server that refuses `initialize`** (a 4xx or a JSON-RPC error) is asked again the modern way: no handshake,
  `MCP-Protocol-Version: 2026-07-28`, and the version and client info in `_meta`. `tools/call` keeps its current
  flow.
- **Refusals in words:** "pd serves no MCP events", "pd has no event x", and so on.

### Places it reaches (ADR-0040)

- **Inside wisp:** triggers (a `trigger` in `wisp.yml`, or `wisp trigger add <name> mcp.event`) and crew wakes (a
  member's `wakes:` is a trigger).
- **From a harness:**
  - wisp's MCP endpoint (`events/list`, `events/poll`) for any Events-capable client;
  - the MCP tools `events_poll` and `connectors_events`;
  - `wisp_events`, as before.
- **CLI:** the generated verbs of `events.poll` and `connectors.events` (ADR-0022).
- **Not in the UI yet:** the trigger editor's source list (`plugins/automations/ui/src/trigger-form.ts` `SOURCE`)
  doesn't offer `mcp.event`. A trigger is written as code, from the CLI or by an agent.

## Consequences

- **A user plugin can serve an MCP extension method** without touching wisp.mcp.
- **Any events-capable MCP server** (PagerDuty, Gmail, a CI) can start a task or wake a crew member with no wisp
  plugin of its own. `linear.issue` and similar sources can move onto this once those servers serve events.
- **ChatGPT, SDK agents and other harnesses** can follow `task.finished`, `pr.opened` or `goal.failed` by poll.
- **wisp declares `2026-07-28`** while serving it statelessly. Only `resultType` is added, and no modern-only field
  is required of clients. A modern client that needs more (`subscriptions/listen`, caching hints) gets what a legacy
  one gets.
- **The SEP's error numbers are provisional.** If they change before the core schema records them, `mcp.rs` and
  `events.rs` change with them.

### Edge cases (each with a test)

- `initialize` without contributions is v1's answer. With them, the extension is declared in `initialize` and in
  `server/discover` (`dispatch::tests`).
- A contributed `tools/*` method is never handed over, and an unserved one is `-32601` (`dispatch::tests`).
- A modern request gets `resultType`. An unknown modern version gets `-32022` with the supported list
  (`dispatch::tests`).
- Polling from `null` returns nothing. The next poll returns the turn's `turn.finished`, and a third returns it no
  more. An unknown type is `-32023` and `events/stream` is `-32026`
  (`serve_events.rs mcp_events_poll_brings_what_happened_after_the_cursor_once`).
- A capped agent polling another task's events gets `-32024` (`serve_events.rs capped_agent_sees_only…`).
- Only durable types are listed, each once (`mcp::tests`).
- A connector's first poll starts from now, later polls key each event once and echo the match, and an unchanged
  cursor returns nothing (`connectors/host/tests/events.rs`).
- `hasMore` is drained in one poll.
- A server with no events says so in words, and a trigger without `match.event` is refused before any request.
- A modern-only server is asked with `MCP-Protocol-Version` and `_meta`.
- Pure parts: the subscription from `match`, the kept cursor (including a server's `null`), occurrences without an
  id left out, RFC 3339 times with offsets, refusals per code (`connectors/core/src/events.rs`).

### Later

- **Push (`events/stream`):** wisp's `Net` is request/response, so it needs a streaming fetch on the client. On the
  server it is the `events:<glob>` topic behind SSE.
- **Webhook (`events/subscribe`):**
  - As a server: `events.sink@1` with Standard Webhooks signing, the challenge, TTL and `deliveryStatus`.
  - As a client: the grid's public URL as the callback.
- **The trigger editor** offers `mcp.event`, with the connector's event types from `connectors.events` and the
  `inputSchema` as the form.
- **Payload schemas** in the catalogue, so `events/list` can carry real `payloadSchema`s.
- **A stdio connector's events**, once wisp calls stdio connectors at all.
