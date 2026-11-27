# ADR-0145: MCP Events both ways, poll and push: an `mcp.event` trigger source, and wisp's log on `events/poll` and `events/stream`

- Status: accepted
- Date: 2026-11-27
- Source: the owner asked what MCP events are and how wisp can use them, then said "do 1 and 2": (1) wisp as an
  Events client (any MCP server's events start a task or wake a crew member) and (2) wisp as an Events server (any MCP
  client reacts to wisp's durable events). The extension is SEP-3415, `io.modelcontextprotocol/events`, merged as
  Final on 2026-11-19. A follow-up the same day: "Add push delivery (events/stream) both ways". Builds on ADR-0043 (one trigger engine, `trigger.source@1`), ADR-0044 (the durable log and
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
  `extension: (identifier, settings)` and a
  `run(ctx, call, MethodRequest { id, params }) -> Result<MethodAnswer, MethodError>`.
  - `MethodAnswer` is a `Result(Value)`, or a `Stream` of whole JSON-RPC messages for a long-lived method. wisp.mcp
    sends a stream as the SSE response of Streamable HTTP: one `data:` frame per message, ended when the method
    ends it or the client goes away (the dropped receiver ends the method at its next send). A stream can only answer
    one message sent alone over HTTP. In a batch, or through `mcp.rpc`, it is refused with `-32026`.
  - `MethodError` carries `code`, `message` and a typed `data`, the way SEP-3415's codes do.
  - `run` gets wisp.mcp's context and the caller's `Call`. A contribution runs its own commands from its own context
    (so its manifest's permissions hold) with that `Call`, so a task's agent's capability set and scoping apply.
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

- **`events/list`** lists the catalogue's **durable** types only, each with `delivery: ["push", "poll"]`. Ephemeral
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
- **`events/stream`** (push) opens by running `events.poll` as the caller, so the type, the cursor, the capability
  and the task scope are checked once, before any stream. A refusal is a plain JSON-RPC error, and no stream opens.
  Then the stream sends:
  - `notifications/events/active {cursor, truncated, heartbeatMs}`, then the backlog after the cursor (500 events a
    read, read again while `hasMore`);
  - each new event as `notifications/events/event`, with `cursor` set to its own seq;
  - `notifications/events/heartbeat {cursor}` whenever `heartbeatMs` passes with no message sent. The client
    suggests the interval, clamped to 5 s to 60 s, 30 s by default. The timer counts from the last message sent,
    because other types' events wake the stream too;
  - `notifications/events/error` for a failed read (the stream stays open), and another `active` with
    `truncated: true` if the cursor fell out of retention.
  - Every message carries `_meta["io.modelcontextprotocol/subscriptionId"]`, the request's id.
  - **No stream polls the log.** The plugin's durable feed handler (`feed`, which already writes the
    `events_view` rows) bumps a `watch` of the newest seq. Each open stream waits on it, then reads the log past its
    cursor.
- **Not yet:**
  - `events/subscribe` answers `Unsupported` (`-32026`, `data: {feature: "deliveryMode", value: "webhook"}`).
  - `events/unsubscribe` answers `NotFound` (`data.kind: "subscription"`).
  - Webhook delivery exists as `events.sink@1`. Moving it onto the SEP's method is later work (below).

### 4. wisp as an Events client (`wisp.connectors`)

- **`connectors.events {id}`** lists a remote connector's event types: every `events/list` page, ten at most.
  A server that answers `-32601` returns `{served: false}`. It is also the MCP tool `connectors_events`.
- **The `mcp.event` trigger source** is a `trigger.source@1` that the engine polls every minute by default. Its
  fields go in `match`, the source-fields convention of ADR-0043:

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

#### Push from a connector (`events/stream`)

- **Push is taken when it's offered and wanted:** the connector's listing (cached 10 minutes) has `push` in the
  event's `delivery`, and the trigger doesn't say `match.delivery: poll`. `delivery` is echoed in the payload, so
  the engine's `match` filter still passes the trigger's events.
- **The engine's poll stays the lifecycle.** The first poll of a subscription opens one `events/stream`, shared by
  every trigger that names the same connector, event and arguments. The stream sends each event to the engine the
  moment it comes (`automations.trigger.push`, so `wisp.connectors` gets `automations:run`). Each event has the same
  key as a polled one, so the engine still fires once.
  - While the stream is open, a poll asks the server nothing. It keeps the stream alive and returns the stream's
    cursor, which lands in the trigger's row, so a restart opens the stream where it was.
  - A stream that no poll touched for three intervals (at least 5 minutes) closes: the trigger was switched off or
    removed.
- **The stream connects through `call::open_stream`**, the same path as every other request (headers, sign-in and
  refresh, the modern fallback). The last request uses `Net::stream` instead of `fetch`.
  - A JSON answer instead of SSE is a refusal, read as one.
  - A stream may stay open for 30 minutes, then it is opened again with its cursor.
- **Liveness:** no message for three heartbeats (as granted in `active`, 25 s asked for) means the stream is dead.
  It is opened again from its cursor after a backoff of 1 s doubling to 30 s, plus a jitter of up to half of it. A
  stream that said something resets the count.
- **When it stops, polling takes over from the stream's cursor**, which is newer than the engine's:
  - five failed opens in a row: polls for 10 minutes, then push is tried again;
  - `terminated`, or a refusal of `events/stream`: polls for an hour.

### Places it reaches (ADR-0040)

- **Inside wisp:** triggers (a `trigger` in `wisp.yml`, or `wisp trigger add <name> mcp.event`) and crew wakes (a
  member's `wakes:` is a trigger).
- **From a harness:**
  - wisp's MCP endpoint (`events/list`, `events/poll`, `events/stream`) for any Events-capable client;
  - the MCP tools `events_poll` and `connectors_events`;
  - `wisp_events`, as before.
- **CLI:** the generated verbs of `events.poll` and `connectors.events` (ADR-0022).
- **Not in the UI yet:** the trigger editor's source list (`plugins/automations/ui/src/trigger-form.ts` `SOURCE`)
  doesn't offer `mcp.event`. A trigger is written as code, from the CLI or by an agent.

## Consequences

- **A user plugin can serve an MCP extension method** without touching wisp.mcp.
- **Any events-capable MCP server** (PagerDuty, Gmail, a CI) can start a task or wake a crew member with no wisp
  plugin of its own. `linear.issue` and similar sources can move onto this once those servers serve events.
- **ChatGPT, SDK agents and other harnesses** can follow `task.finished`, `pr.opened` or `goal.failed` by poll, or as
  they happen by push.
- **One open stream holds one HTTP connection** on both sides. On the server, a quiet stream costs a task that wakes
  on each durable batch and does one indexed read. There is no cap on concurrent streams yet; per-caller limits
  would answer `ResourceExhausted` (`-32025`).
- **A wisp that subscribes to its own events** (a connector to its own `/api/mcp`, an `mcp.event` trigger on
  `turn.finished`) would start a task on each task's end, its own included. The `depth` and lineage guards of
  ADR-0043 don't cross the MCP boundary, so the trigger's `rate`, `concurrency` and `budget` are the brakes.
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
  more. An unknown type is `-32023` and `events/subscribe` is `-32026`
  (`serve_events.rs mcp_events_poll_brings_what_happened_after_the_cursor_once`).
- **A stream on `wisp serve`** sends `active` with the subscription id and the granted heartbeat, then the turn's
  `turn.finished` as it happens (its cursor equals its `eventId`), then a heartbeat. A type that isn't kept is
  refused before any stream (`-32023`), and a stream inside a batch is `-32026`. wisp's own client lists wisp's own
  events, with push offered (`serve_events.rs mcp_events_stream_pushes_each_event_as_it_happens`).
- **Stream messages** carry the subscription id, and events carry their cursor. The heartbeat is clamped
  (`mcp::tests`).
- **A connector's stream** opens at the first poll (from now). Each event reaches the engine at once, and a poll
  meanwhile asks nothing and returns the stream's cursor. A dropped stream is opened again from its cursor. After
  `terminated`, polling carries on from the stream's cursor, not the engine's older one. `match.delivery: poll`
  never streams (`connectors/host/tests/events.rs`).
- **Pure parts of push:** offers and `wants_push`, `delivery` echoed, every frame kind read, a stream's error
  answer read as terminated, backoff (`connectors/core/src/events.rs`).
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

- **`wisp mcp` (stdio)** forwards only tools to the server today. It could forward `events/*`, a stream as
  notifications on stdout, cancelled with `notifications/cancelled`.
- **A limit on concurrent streams per caller** (`-32025`).
- **Webhook (`events/subscribe`):**
  - As a server: `events.sink@1` with Standard Webhooks signing, the challenge, TTL and `deliveryStatus`.
  - As a client: the grid's public URL as the callback.
- **The trigger editor** offers `mcp.event`, with the connector's event types from `connectors.events` and the
  `inputSchema` as the form.
- **Payload schemas** in the catalogue, so `events/list` can carry real `payloadSchema`s.
- **A stdio connector's events**, once wisp calls stdio connectors at all.
