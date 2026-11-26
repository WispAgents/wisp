# ADR-0134: `Net` streams a response body, so an SSE connector is checked over its event stream

- Status: accepted
- Date: 2026-11-26
- Source: connectors audit round 3 (CONN-4), review of PR #974.

## Context

- A remote connector can speak the 2024 SSE transport (`"type": "sse"`, kept since CONN-4 as the spec's
  `transport: "sse"`): a GET with `Accept: text/event-stream` stays open, its first event (`endpoint`) names where
  to POST, each POST is answered `202`, and the answers come back on the open GET.
- The kernel's one HTTP client (`wisp_kernel::services::Net`, plan §3.1 rule 8: plugins never open their own) had
  only `fetch`, which returns a whole body. A stream that stays open never ends, so the connectors check couldn't
  read it: Test/Check on an SSE connector reported the streamable POST's failure (a 404 or 405) and the row read
  "Didn't start" although runs reached the server, since the harnesses speak SSE themselves.

## Decision

1. **`Net::stream(req) -> NetStream`**: the same request, its status and headers at once and its body as a stream of
   chunks (`futures::stream::BoxStream<'static, Result<Vec<u8>>>`, an `unavailable` error ends it). Dropping it
   closes the connection; `timeout_ms` bounds the whole stream. The install's `ReqwestNet` implements it with
   reqwest's `bytes_stream` (the `stream` feature was already on). The trait's default reads the whole body with
   `fetch` and gives it as one chunk, so every other `Net` (the grid's, a test's fake) keeps compiling and answering.
2. **The connectors check asks an SSE server over SSE** (`pane::probe_sse`): when the spec's transport is `sse`, or
   when the streamable `initialize` POST gets a 404 or 405 (a server that only speaks the 2024 transport). It opens
   the stream, reads `endpoint` (`rpc::endpoint_url`: read against the stream's URL, and only on its origin, so a
   server can't have the connector's headers sent elsewhere), POSTs `initialize`, `notifications/initialized` and
   `tools/list` there, and feeds every `message` event (and any answer a POST's own reply carries) to the probe's
   state machine. One minute in all (`PROBE_MS`), as a local check.

## Consequences

- A plugin can read a long-lived HTTP response (an event stream) through the kernel's one client; no plugin opens
  its own.
- A `Net` implementation that can't stream still serves `stream`, by buffering: right for a short reply, never ending
  for an open event stream. A test that checks an SSE server gives its fake a real `stream` (`tests/sse.rs`).
- Reaches: the connectors pane's Test and the check after add, edit and import. Runs are unchanged: a harness that
  speaks SSE gets the server as `"type": "sse"`; Codex, which doesn't, gets no SSE connector (CONN-4).
