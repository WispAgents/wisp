# Full Audit: States, Events, Sync & Follow-ups (Desktop, Web & Mobile)

**Date:** 2026-11-22  
**Scope:** End-to-end audit of live state propagation, event delivery, direct vs. queued follow-ups, task lifecycles, and reconnect handling across Rust (`wisp-store`, `wisp-sync`, `wisp-api`, `plugins/runtime`, `plugins/later`, `plugins/goal`) and TypeScript/Solid (`packages/sync`, `packages/client`, `packages/ui`, `packages/shell`, `apps/web`, `mobile/web`, `plugins/*/ui`).

---

## 1. Root Causes: Task Updates Missing Until Reload (Desktop & Mobile)

### 1.1 Multi-topic duplicate `Patch`/`Append` delivery → `drifted("list")` → `POST unsub`/`sub` race permanently unsubscribing `"list"`
- **Path:** `crates/wisp-sync/src/server.rs`, `packages/client/src/peers.ts`
- **Root cause:**
  1. `session` rows publish to both `"list"` and `"session:<id>"`. `to_wire` fanned every change into two `ChangeRow`s, and `SyncServer::committed` pushed both `Patch` ops into the same connection's outbox and same SSE frame.
  2. `EntityStore` shares one `Cell` per `(kind, peer:id)`. Whenever an optional field was removed (e.g., `summary.live`, `ask`, `forLater`, `waitsOn`), op #1 (`session:<id>`) succeeded and op #2 (`list`) threw `Error("nothing to remove at ...")` (and `Add /-` duplicated array items).
  3. `applyServer` marked `"list"` drifted; `SyncClient` emitted `unsub("list")` and `sub("list")` back-to-back.
  4. `packages/client/src/peers.ts` fired both `POST /api/events` concurrently with no ordering and swallowed errors. When `sub` arrived before `unsub`, `SyncServer` treated `sub` as a no-op (already `Live`), then `unsub` deleted `"list"` from `c.subs`. From that point on, zero list updates arrived until a full reload.
- **Fixes:**
  - `crates/wisp-sync/src/server.rs`: Deduplicate `Change::Patch` and `Change::Append` per `(conn, seq, entity, id)` across `Live` subscriptions (delivering once on the longer-lived topic, e.g., `list` over `session:<id>`, while keeping `Upsert`/`Del` on every topic and keeping `Loading` sub buffering intact).
  - `packages/client/src/peers.ts`: Serialize `transport.post()` per peer via a FIFO promise chain (`postQueue`), and reopen the stream if a `POST` fails while the same stream is still live.

### 1.2 Live streamed `event` rows (`ChangeOp::Append`) dropped by `TaskView` and `Compare`
- **Path:** `packages/shell/src/frame/turn-events.ts`, `packages/shell/src/frame/TaskView.tsx`, `packages/shell/src/frame/Compare.tsx`, `plugins/crew/ui/src/subagents.ts`
- **Root cause:** Snapshot events are individual objects (`id: "1"`, `id: "2"`), while live `w.append("event", rows)` stores an array under `peer:<session_id>`. `eventsByTurn()` and `Compare.tsx` read `e.turn_id` directly on array entries (`undefined`), hiding all live tool steps, transcript entries, ticker updates, and action counts until reload.
- **Fix:** Flatten both individual and array `'event'` entries in `eventsByTurn`, `Compare.tsx`, and `subagents.ts`, deduplicating by `(turn_id, seq)`.

### 1.3 Stale socket backpressure (`c.buffered`), 15 s `SubState::Waiting` stalls, and zombie connections on `try_send` failure
- **Path:** `crates/wisp-api/src/v2_actor.rs`, `crates/wisp-api/src/v2.rs`, `crates/wisp-sync/src/flush.rs`
- **Root cause:** `v2_actor.rs` only refreshed `Input::Drained` on `SnapshotRows` and `Tick` (not `Post`, `Committed`, or `CommandResult`), and `wake_at()` ignored `SubState::Waiting`, stalling new task opens for up to 15 s (next heartbeat). When `try_send` failed, `carry()` removed the stream from `Actor::conns` without feeding `Input::Close` to `SyncServer`, leaving zombie connections in `by_client` that returned `403 forbidden` on subsequent `POST /api/events`.
- **Fixes:** Refresh `buffered_bytes` from `s.queued` before `Post`, `Committed`, `CommandResult`, `SnapshotRows`, and `Tick`; decrement `queued` after `out.send(b).await`; wake at `flush_ms` when any sub is `SubState::Waiting`; feed `Input::Close` to `SyncServer` on `try_send` failure.

### 1.4 Non-atomic multi-transaction paged `list` snapshots
- **Path:** `crates/wisp-api/src/v2_pages.rs`, `crates/wisp-sync/src/server.rs`, `crates/wisp-sync/src/conn.rs`
- **Root cause:** `read_short` read each page of `list` in its own `store.read` while stamping every page with Page 1's `head`. A task updated mid-scan jumped above Page 1's keyset cursor (omitted from later pages → missing-row patch drift) or was read post-update and patched a second time by `sub.buffer`.
- **Fix:** Reconcile `r.changed_since(topic, head)` inside the final page's transaction (`end_head`), keep Page 1's wire `seq` (`Sub::page_seq`) for client page stitching while filtering `sub.buffer` by `seq > end_head`, and emit `Change::Del` ops for rows deleted mid-scan.

### 1.5 Missing foreground/resume/online recovery (Desktop, Web, Mobile) & WebKit HTTP/1.1 connection leak
- **Path:** `packages/sync/src/client.ts`, `packages/sync/src/types.ts`, `packages/client/src/peers.ts`, `packages/client/src/http.ts`, `apps/web/src/main.tsx`, `mobile/web/src/boot.ts`, `mobile/web/src/peers.ts`, `mobile/web/src/transport.ts`, `packages/shell/src/watch.ts`
- **Fixes:**
  - Added `{ kind: 'resume' }` to `SyncClient` and `peers.resume()` (reopens offline streams immediately and drops/reopens streams silent for ≥ 1 keepalive without disturbing healthy streams).
  - Wired `platform.onResume`, `visibilitychange` (`visible`), and `window` `online` in both `apps/web/src/main.tsx` and `mobile/web/src/boot.ts`.
  - `mobile/web/src/peers.ts`: On `onTunnel('ready')`, reconnect whenever the peer isn't `asleep`/`waking` (not only when already `unreachable`).
  - `mobile/web/src/transport.ts`: Clear `open` in `finally` of `connect()` so `request()`/`events()` never reuse a dead SSH port/token.
  - `packages/client/src/http.ts`: Cancel `reader` on `signal` `'abort'` so aborted SSE streams immediately release WKWebView HTTP/1.1 connection slots.
  - `packages/shell/src/watch.ts` & `shell.tsx`: Make `watchCoreTopics` react to `client.home` changes.

### 1.6 Sync, EntityStore, Solid Reactivity & UI Staleness Fixes
- `packages/sync/src/client.ts` & `packages/client/src/peers.ts`: Notify `SyncClient` (`{ kind: 'evicted', topic }`) when `dropTopic` evicts an unwatched topic after 30 s so reopening requests a fresh snapshot; reset `#cursor = null` on `!resume` in `#welcome`.
- `packages/client/src/entities.ts`: Read `r.sig` via `untracked()` in `recompute`; fix same-batch `del` + `upsert` dropping rows permanently from `c.members`; key `snapshot()` `seen` by `${kind}\0${key}` (prevents ghost `queue_item`/`watch` rows); drop acked (`settleAt !== undefined`) pending overlays in `resetPeer()`.
- `packages/client/src/first-paint.ts`: Serialize chunked `flush()` calls so concurrent flushes cannot overwrite a newer cursor with an older one.
- `packages/client/src/commands.ts` & `peers.ts`: Queue `mutate` for peers not yet added (replayed on `add()`) and clean up pending commands/overlays on `peers.remove()`.
- `packages/client/src/selectors.ts`: Delete old object references from `pos` in `sortBy`.
- `packages/ui/src/core/signal.ts` & `plugins/shell-list/ui/src/ui/follow.ts`: Detach active alien-signals subscriber inside Solid `createComputed` (`useSignal` and `follow`) so child subscriptions are never killed by outer alien re-runs.
- `packages/shell/src/frame/TurnTranscript.tsx`: Read `props.a` and `props.changes` reactively inside `<Step>` so live `tool_output` and diff figures appear without reload.
- `packages/shell/src/frame/Panes.tsx`: Wrap `useSession()` with `useDerived()` so `MainPane`/`SidePane` extensions track live updates.
- `plugins/shell-list/ui/src/model/wire.ts` & `plugins/shell-home/ui/src/notify.ts`: Apply top-level columns over `summary` in `flatSession()`/`flatRow()` so column-level and optimistic patches (`/title`, `/state`, `/unread`, `/archived`) are never overwritten by stale `summary`.
- `plugins/shell-list/ui/src/model/sections.ts`: Include `state === 'error'` in `needs` / `needsYou` so errored tasks never disappear from the list.
- `plugins/shell-task/ui/src/task-context.tsx`, `side/ChangesPane.tsx`, `side/PrPane.tsx`, `side/SidePanel.tsx`, `side/FilePane.tsx`, `foot/WatchOpen.tsx`: Support snake_case `session_id`/`started_at` in `turnsOf`; refresh `ChangesPane` and `PrPane` in place on task/PR updates; fix `PrPane.canAsk()` to check `state`; use `useDerived` for `SidePanel.extra`; reset `FilePane` signals on `path` change; reset `WatchOpen` `seen` on `key()` change.
- `plugins/shell-list/ui/src/state.ts`: Prefer `home` peer's `projectOrder` setting row.

---

## 2. Root Causes: Direct (Non-Queued) Follow-ups Not Sent Directly

### 2.1 `from_queue` blocked direct/interrupting follow-ups
- **Path:** `plugins/runtime/host/src/finish.rs`, `plugins/runtime/host/src/runtime.rs`
- **Root cause:** `release_run` unconditionally added `sid` to `from_queue` whenever `next.is_some()` — including interrupting (`now: true`) follow-ups and lead reports (`reports: true`). `runtime.rs` then passed `from_queue` into `run_for_follow_up`, downgrading `Run::Running { resumable: true }` to `resumable: false`, forcing subsequent direct follow-ups into the queue instead of interrupting.
- **Fixes:**
  - `finish.rs`: Only mark `from_queue` when `next` is a non-`now`, non-`reports` queued item.
  - `runtime.rs`: Never apply `from_queue` downgrade when `a.queue == Some(false)` (explicit direct send), while preserving Audit I3 ordering for implicit `a.queue == None` CLI calls.

### 2.2 Deferred interrupt never fired once `resumable` became `true`
- **Path:** `plugins/runtime/host/src/runtime.rs`, `run.rs`, `drive.rs`, `ops.rs`, `finish.rs`
- **Root cause:** When a direct follow-up (`queue: false`) or `SendNow` arrived while a turn was `starting` or before `HarnessEvent::Session` reported a `resume_id`, it queued with `now: true`, but neither `spawn_run` nor `Runtime::resumable()` checked for a waiting direct follow-up — so `Ctl::Interrupt` was never sent and the follow-up sat in the queue for the entire turn.
- **Fix:** Added `pending_interrupt: Arc<Mutex<HashSet<String>>>` on `Runtime`. Set when a direct follow-up hits `Run::Running { resumable: false }` or `SendNow::Next`; consumed and immediately sends `Ctl::Interrupt` in `spawn_run` (if `resumable`) and `Runtime::resumable()`; cleared on `stop_run`, `delete`, `release_run`, and `queue_cancel` (when no `now` items remain).

### 2.3 Frontend follow-up sending, optimistic queue patches, and `later.reply` interception
- `packages/shell/src/frame/TaskView.tsx`: Explicitly pass `queue: queues()` on both `send()` and `reply()` (previously omitted `queue: false`); add `.catch(() => setChecking(false))` on `holds.check`; allow sending attachments without text; match `parent_id` against both `key` and `bare`.
- `packages/shell/src/frame/commands.ts`: Fix `queuedOps` to upsert a `queue_item` entity on real server rows instead of creating `/body/queue` (which hid existing queued items).
- `plugins/runtime/host/src/runtime.rs`: Return the actual inserted `item_id` in `FollowedUp.queued` for both `Plan::Queued` and `Plan::Interrupt`.
- `plugins/later/ui/src/reply.ts`: Check snake_case `parent_id`, `state === 'waiting'`, `body.ext['wisp.runtime'].ask`, and `body.ext['wisp.later'].kind` so replies to waiting/subagent/reminder tasks starting with time words aren't hijacked into `later.add`.

---

## 3. Additional Backend State, Store, Sync & Plugin Bugs Fixed

- **Store (`crates/wisp-store`):**
  - `changes.rs`: Fixed multi-page (> 10,000 changes) `changed_since` ordering/deduplication.
  - `events.rs`: Preserved newest row (`seq < max`) in `trim_on` so SQLite rowid never resets to 1; moved rollback `forget()` synchronously onto the writer thread.
  - `store.rs`: Added RAII `ROLLBACK` guard in `Store::read` so panics never poison pooled readers.
  - `reads.rs`: Added `Reader::key_of(kind)` for accurate entity key resolution.
- **API & Sync (`crates/wisp-api`, `crates/wisp-sync`, `crates/wisp-kernel`):**
  - `v2_store.rs`: Fixed `oldest_seq()` off-by-one (`saturating_sub(1)` matching `can_resume`); resolved non-`"id"` primary keys via `Reader::key_of`; advance `SyncServer::core.head` via `Input::Advance` when a commit emits no wire ops.
  - `conn.rs`: Included `sub.buffer` oldest seq in `watermark()` so acks and SSE `id:` never advance past buffered loading ops.
  - `idem.rs`: Stopped caching retryable 5xx / `error.retry == true` responses for 10 min.
  - `egress.rs` & `env_view.rs`: Added `RowView::strip_patch` to strip `/body/envHide` patch ops on `watch`/`watch_page`, and called `drop_kept` in `View::rows` when masking is off.
- **Runtime (`plugins/runtime/host`):**
  - `ops.rs`: Fixed `Runtime::settle` (lock before `run_state`, sort `after_steps`, spawn `l.run_after` so chained after-turn steps run); added `task_lock` to `release_as`.
  - `finish.rs` & `more/report_back.rs`: Prevented permanent `SessionState::Running` stuck state when `start_claimed` fails or `claim_next` returns `None`.
  - `runtime.rs`: Reconciled `summary` in `write_session` when `reconcile_ask` mutates `row` inside the transaction.
- **Later (`plugins/later/host` & `ui`):**
  - `lib.rs` & `calls.rs`: Send `calls::follow_up` before deleting entry in `fire()` and restore entry on error; implement `EditPlan { now: true }`; separate `PlanAction::Run` vs `PlanAction::Missed` and create reminder/missed cards via `runtime.card`; fix `MakePlan` `card` check (`meta.card`, not `state == Done`); apply `new_state` in `remind()`; subscribe to `TurnStarted` (runs `rules::on_turn`) and `SessionDeleted`.
  - `commands.ts` & `LaterFoot.tsx`: Fix `editPlan` optimistic patches for enveloped `later_plan` rows; use scoped 30 s interval in `LaterFoot`.
- **Goal (`plugins/goal/host/src/judging.rs`):**
  - Skip `e.inner` turns on `TurnEnded` and pass `queue: Some(true)` on `FollowUp`.
