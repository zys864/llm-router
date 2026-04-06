# Access Log Design

## Goal

Add structured interface-call logging for incoming proxy requests, upstream
attempts, and terminal request outcomes without recording full prompts or full
responses.

## Scope

This design includes:

- A dedicated access-log subsystem separate from usage logging
- Append-only JSONL logging to a configurable local file
- Structured events for request start, upstream attempts, and request finish
- Request-summary fields that describe payload shape without storing prompt text
- Tests that verify event coverage and redaction behavior

This design does not include:

- Replacing or removing the existing usage log
- Full prompt or response body persistence
- External log sinks such as databases, Kafka, or hosted observability tools
- A new admin API for querying access logs

## Recommended Approach

Keep usage logging as the terminal accounting log and add a second,
purpose-built access log for request tracing and debugging.

This is the right fit for the current codebase because usage logging already
feeds quota recovery and admin aggregation. Mixing request-trace events into the
usage log would blur responsibilities and make both debugging and aggregation
harder. A separate access log preserves a clear boundary:

- `usage` remains one terminal record per request for accounting and summaries
- `access log` becomes a multi-event trace for auditing and troubleshooting

## Architecture

### New Module

Add `src/access_log.rs` with:

- `AccessLogEvent`
- `RequestSummary`
- `UpstreamAttemptSummary`
- `RequestFinishedSummary`
- `AccessLogger`

`AccessLogger` should mirror the current `UsageLogger` shape: an optional JSONL
file target backed by `Arc<Mutex<File>>` so concurrent requests append safely in
order.

### App Wiring

Add `ACCESS_LOG_PATH` to configuration and load it into `AppConfig` as
`access_log_path: Option<String>`.

Extend `AppState` with an `access_logger: AccessLogger`, initialized in
`build_app` alongside the existing `usage_logger`.

If `ACCESS_LOG_PATH` is unset, access logging is disabled and logger calls
become no-ops.

## Event Model

Each request may emit multiple access-log events. The event model should be
explicit rather than overloading a generic blob field.

### `request_started`

Written after request authentication and route resolution, before any upstream
attempt runs.

Fields:

- `timestamp`
- `event = "request_started"`
- `request_id`
- `method`
- `path`
- `api_kind`
- `caller_id`
- `public_model`
- `stream`
- `request_summary`

### `upstream_attempt`

Written once per route-target attempt after that attempt completes, whether it
ends in success or failure.

Fields:

- `timestamp`
- `event = "upstream_attempt"`
- `request_id`
- `attempt`
- `provider`
- `upstream_model`
- `public_model`
- `result`
- `latency_ms`
- `error_kind`
- `error_message`

`error_message` should be a short normalized summary, not a raw upstream body.

### `request_finished`

Written once per request on terminal success or terminal failure.

Fields:

- `timestamp`
- `event = "request_finished"`
- `request_id`
- `api_kind`
- `caller_id`
- `public_model`
- `final_provider`
- `attempts`
- `stream`
- `status`
- `status_code`
- `latency_ms`
- `input_tokens`
- `output_tokens`

For failures where no provider returns a final response, `final_provider` may be
absent and token fields may be absent.

## Request Summary Policy

The access log should include payload summaries that help with debugging while
avoiding prompt persistence.

`RequestSummary` should capture:

- `message_count`
- `system_message_count`
- `user_message_count`
- `assistant_message_count`
- `other_message_count`
- `input_text_chars`
- `has_system_prompt`
- `has_temperature`
- `has_top_p`
- `has_max_tokens`

The following data must not be logged:

- Raw message text
- Full prompt arrays
- Image URLs or binary payloads
- Tool call arguments or tool outputs
- Full upstream response bodies

This summary is intentionally lossy. It exists for request-shape inspection, not
content replay.

## Data Flow

### Request Start

In both `chat_completions` and `responses` handlers:

1. Authenticate the caller
2. Resolve the route plan
3. Build a `RequestSummary` from the incoming payload
4. Append a `request_started` event
5. Continue into upstream execution

### Upstream Attempts

Inside the route-attempt loop:

1. Capture a per-attempt start timestamp before calling the provider
2. On success, append `upstream_attempt` with `result = "success"`
3. On failure, append `upstream_attempt` with `result = "failure"` and a short
   normalized error summary

Fallback behavior stays unchanged. Access logging observes the attempt chain; it
does not change routing decisions.

### Request Finish

Before returning the final downstream response, append `request_finished` with:

- terminal status
- HTTP status code
- total request latency
- final provider, if known
- usage tokens, if known

For streaming success, write `request_finished` when the upstream stream is
successfully established, using the known route/provider metadata and any usage
that is available at that moment. If future work adds terminal stream callbacks,
that can refine logging later, but this phase should stay consistent with the
current handler lifecycle.

## Error Handling

Access-log write failures must never fail the API request.

If access-log serialization or file I/O fails:

- emit `tracing::warn!` with the request context when available
- continue handling the request normally

This is important because log-path issues are operational problems, not request
correctness problems. The proxy must not become unavailable due to local logging
failure.

## Relationship To Usage Logging

`usage log` and `access log` remain separate on purpose.

`usage log`:

- one terminal record per request
- used for quota recovery, pricing summaries, and admin aggregation

`access log`:

- one or more records per request
- used for request tracing, fallback visibility, and debugging

No existing admin or quota feature should be changed to depend on access logs.

## Configuration

Add:

- `ACCESS_LOG_PATH`

Behavior:

- unset: disabled
- set to a file path: append JSONL events to that file

The README and `.env.example` should eventually document this variable, but that
documentation work belongs to the implementation plan rather than this design
spec.

## Testing Strategy

### Unit Tests

- `RequestSummary` counts roles and character totals without storing message text
- `AccessLogger` appends JSONL records when enabled
- disabled `AccessLogger` is a no-op

### Integration Tests

- successful request writes `request_started`, `upstream_attempt`, and
  `request_finished`
- fallback request writes multiple `upstream_attempt` events
- terminal failure still writes `request_finished`
- logged JSON does not contain the original prompt text

Integration tests should follow the current test pattern: boot the app with a
temporary log path and mocked upstream providers, then inspect the written JSONL
file.

## Success Criteria

This work is complete when:

- the proxy can be configured with `ACCESS_LOG_PATH`
- each handled request writes a start event and a finish event
- each upstream route attempt writes an attempt event
- request summaries expose shape metrics without storing prompt text
- access-log failures do not break API responses
- tests cover success, fallback, failure, and redaction behavior
