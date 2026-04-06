# LLM Proxy Phase 2 Design

## Goal

Extend the MVP proxy into a more OpenRouter-like service by adding real upstream
streaming support, model capability metadata, fallback routing, inbound API key
authentication with quotas, and append-only usage logging.

## Scope

This phase includes:

- Real provider streaming passthrough for OpenAI, Anthropic, and Gemini
- Model metadata for capability flags and routing priority
- Fallback routing across configured upstream targets
- Optional retry on upstream failure before exhausting fallback targets
- Inbound proxy API key authentication
- Per-key quota enforcement
- Append-only JSONL usage logging
- Updated tests for streaming, routing, auth, quota, and logging

This phase does not include:

- Persistent quota state across process restarts
- Billing calculations or price schedules
- Admin APIs for key management
- Database-backed usage storage
- Provider-native tools, multimodal inputs, or file APIs
- Adaptive routing based on latency or price

## Decomposition

The remaining work spans four subsystems, but they can still be implemented in a
single sequential phase because they attach to the current architecture at clear
seams:

1. Provider streaming attaches to `src/providers/*`, `src/domain/response.rs`,
   and `src/sse.rs`
2. Model metadata and fallback routing attach to `src/config.rs`,
   `src/models.rs`, and `src/router.rs`
3. API key auth and quota attach to the request entry layer in `src/api/*`
   through shared app state
4. Usage logging attaches after provider completion or stream termination

This keeps the API surface stable while expanding internal capabilities.

## Recommended Approach

Use the existing OpenAI-compatible handlers as the stable front door and extend
the internal seams behind them. The proxy should continue to normalize requests
into a shared internal model, but routing must resolve to a list of candidate
upstream targets rather than exactly one target. Provider adapters should return
either a unified non-streaming response or a typed stream of provider events.

This approach avoids rewriting the public API while making the routing and
observability layers significantly more capable.

## Architecture

### Request Path

1. Incoming request enters an OpenAI-compatible handler
2. Auth middleware validates the proxy API key when auth is configured
3. Quota guard checks whether the authenticated caller may spend another request
4. Handler resolves the public model name into a candidate route list
5. Router attempts the first target and, on eligible failure, falls through to
   the next target
6. Provider adapter executes non-streaming or streaming upstream call
7. Usage tracker records the request outcome and token counts
8. Response is normalized back to OpenAI-style JSON or SSE

### New Core Components

- `src/auth.rs`
  Parses inbound bearer tokens and resolves them to configured callers.
- `src/quota.rs`
  Holds in-memory quota counters keyed by caller ID.
- `src/usage.rs`
  Defines usage records and append-only logging helpers.
- `src/middleware.rs`
  Provides auth and request-context middleware for `axum`.
- `src/providers/streaming.rs`
  Shared utilities for parsing upstream SSE payloads into internal stream
  events.

Existing files that expand in responsibility:

- `src/config.rs`
  Loads proxy keys, quotas, usage log path, and richer model definitions.
- `src/models.rs`
  Stores model capabilities and ordered fallback targets.
- `src/router.rs`
  Produces candidate route chains and fallback behavior.
- `src/app_state.rs`
  Owns auth config, quota state, and usage logger.
- `src/domain/request.rs`
  Carries caller context and route attempt metadata.
- `src/domain/response.rs`
  Carries streaming event payloads and final usage information.

## Configuration

### Model Configuration

The MVP `MODEL_MAPPINGS` string is no longer rich enough. This phase should add
a JSON-based model catalog config while keeping the simple mapping format
available as a fallback for local development.

Recommended environment variables:

- `MODEL_CONFIG_PATH`
- `PROXY_API_KEYS_PATH`
- `USAGE_LOG_PATH`

`MODEL_CONFIG_PATH` should point to a JSON file with entries like:

```json
[
  {
    "public_name": "claude-sonnet-4",
    "capabilities": {
      "chat_completions": true,
      "responses": true,
      "streaming": true
    },
    "targets": [
      {
        "provider": "anthropic",
        "upstream_name": "claude-sonnet-4-20250514",
        "priority": 100
      },
      {
        "provider": "openai",
        "upstream_name": "gpt-4.1",
        "priority": 50
      }
    ]
  }
]
```

Targets are ordered by descending priority. The router should try targets in
that order unless a request explicitly needs a capability the target does not
support.

### Proxy API Key Configuration

`PROXY_API_KEYS_PATH` should point to a JSON file containing caller records:

```json
[
  {
    "id": "team-alpha",
    "api_key": "lr_live_team_alpha",
    "max_requests": 10000
  }
]
```

If no proxy key file is configured, the service remains open for local
development. If the file exists, every request to `/v1/*` except `/healthz`
requires `Authorization: Bearer <key>`.

### Usage Log Configuration

`USAGE_LOG_PATH` defines the JSONL file used for append-only request records. If
unset, usage logging is disabled. If set, the service creates the file if it
does not exist and appends one JSON line per completed request or terminal
stream outcome.

## Routing and Fallback

The router now resolves a public model to a route plan:

- Public model metadata
- Ordered candidate targets
- Requested capability set

Fallback rules:

- Validation errors do not trigger fallback
- Auth/config errors do not trigger fallback
- Upstream `5xx`, network failures, and timeouts may trigger fallback
- Streaming fallback is only allowed before downstream bytes have been emitted

Non-streaming requests may retry the next target after a failed upstream
attempt. Streaming requests may only fallback if the first target fails before
yielding any stream event. Once streaming has started, the proxy must terminate
the downstream stream on failure and log the partial failure.

## Real Streaming

The MVP currently turns a completed upstream response into synthetic stream
chunks. This phase replaces that with actual upstream stream consumption.

Provider responsibilities:

- OpenAI adapter reads SSE chunks from upstream `/v1/chat/completions` or
  `/v1/responses` with `stream=true`
- Anthropic adapter reads event streams from `/v1/messages` with stream enabled
- Gemini adapter reads chunked generate-content stream responses

Each adapter maps provider-specific events into shared internal stream variants:

- `StreamStarted`
- `TextDelta`
- `UsageDelta`
- `Completed`
- `Errored`

The proxy SSE encoder remains responsible for OpenAI-compatible downstream
formatting, but it should now consume a live async stream rather than a finished
`Vec`.

## Authentication and Quota

Authentication should be implemented as `axum` middleware so handlers do not
need to duplicate auth checks. The middleware resolves the caller identity and
stores it in request extensions for handlers and usage logging.

Quota behavior for this phase:

- Count one request against `max_requests` when the upstream attempt starts
- Reject with `429` if the caller has exhausted quota
- Keep counters in memory for the current process lifetime

This is intentionally simple. The design leaves room for token-based quotas
later, but this phase only needs request-count enforcement.

## Usage Logging

Every completed request should emit a JSONL record with:

- Timestamp
- Request ID
- Caller ID if authenticated
- Public model name
- Selected provider
- Fallback attempt count
- HTTP API kind: `chat_completions` or `responses`
- Stream or non-stream flag
- Final status: `success`, `timeout`, `upstream_error`, `quota_rejected`,
  `auth_rejected`, `stream_aborted`
- Usage metrics when available

For streaming, log on terminal success or terminal failure, not for every chunk.

## API Behavior Changes

### `GET /v1/models`

The models response should expose richer metadata derived from the model
catalog. The exact OpenAI-compatible schema can remain minimal, but each model
entry should at least reflect whether the model is backed by streaming-capable
targets.

### `POST /v1/chat/completions`

The handler behavior remains the same from the client point of view, but
streaming now proxies real upstream deltas. If fallback routing is used for a
non-stream request, only the winning attempt is reflected in the final response.

### `POST /v1/responses`

The responses API should gain the same auth, quota, fallback, and logging
behavior as chat completions. Streaming support should be real rather than
synthetic.

## Error Handling

Add new normalized API errors:

- `authentication_error` for missing or invalid proxy keys
- `rate_limit_error` for quota exhaustion
- `server_error` for exhausted fallback chain

When fallback fails across all candidates, the final error should preserve the
last meaningful upstream message while logging the full attempt chain
internally.

## Testing Strategy

### Unit Tests

- Model catalog parsing with capabilities and target priorities
- Router route-plan construction and fallback selection
- Auth key lookup and quota counting
- Usage log record serialization
- Provider stream event parsing for each provider
- SSE encoding from live internal event streams

### Integration Tests

- Real streaming passthrough from mocked OpenAI upstream
- Real streaming passthrough from mocked Anthropic upstream
- Real streaming passthrough from mocked Gemini upstream
- Fallback from first failing upstream target to second target
- Invalid proxy key returns `401`
- Exhausted quota returns `429`
- Usage log file receives one terminal record per request
- Models endpoint exposes configured metadata-derived fields

## Risks and Constraints

- Provider streaming protocols differ materially; each adapter needs carefully
  bounded parsing code
- Streaming fallback is limited by protocol reality; once bytes are sent,
  automatic retry is no longer safe
- In-memory quotas are process-local and reset on restart; that is acceptable in
  this phase but must be documented
- Usage logging must be append-only and resilient to concurrent requests; use a
  shared async file writer or append lock

## Success Criteria

This phase is complete when:

- Real upstream streaming works end-to-end for the three supported providers
- Fallback routing succeeds for non-stream requests and pre-stream failures
- Proxy bearer auth and request quotas are enforced
- Usage is logged to local JSONL when enabled
- The full test suite covers the new behavior and passes
