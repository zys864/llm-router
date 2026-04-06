# OpenRouter-Style LLM Proxy MVP Design

## Goal

Build a Rust service that exposes OpenAI-compatible APIs and routes requests to
multiple upstream LLM providers. The first version focuses on a minimal usable
proxy that supports OpenAI, Anthropic, and Gemini behind a single API surface.

## Scope

This MVP includes:

- `GET /v1/models`
- `POST /v1/chat/completions`
- `POST /v1/responses`
- Model aliasing from public model names to provider-specific model names
- Provider selection based on configured model mappings
- Non-streaming and streaming request handling
- Unified error responses
- Config-driven upstream API keys and timeouts
- Unit and integration tests with mocked upstream providers

This MVP does not include:

- Billing, balances, quotas, or usage accounting
- Multi-tenant auth or per-user API keys
- Admin UI or dashboard
- Automatic failover across multiple providers
- Retry policies beyond a simple timeout boundary
- Provider-native features that cannot be represented cleanly in the unified API

## Architecture

The service presents an OpenAI-compatible HTTP layer to clients. Incoming
requests are validated, assigned a request ID, converted into a unified internal
request model, and then routed to a provider adapter based on the configured
model alias. Provider adapters translate the internal request into upstream
provider protocol, execute the HTTP call, and normalize the result back into an
OpenAI-style response body or SSE stream.

The design keeps protocol compatibility concerns in the API layer and isolates
provider-specific logic in adapters. This prevents request handlers from
accumulating provider branches and makes it straightforward to add features such
as routing rules, fallbacks, or usage tracking later without rewriting the HTTP
surface.

## Proposed Module Layout

- `src/main.rs`
  Starts the server, loads configuration, wires shared state, and registers
  routes.
- `src/config.rs`
  Loads environment variables and static configuration into typed structs.
- `src/app_state.rs`
  Defines shared application state passed into handlers.
- `src/api/mod.rs`
  Exposes API route wiring.
- `src/api/models.rs`
  Implements `GET /v1/models`.
- `src/api/chat_completions.rs`
  Implements `POST /v1/chat/completions`.
- `src/api/responses.rs`
  Implements `POST /v1/responses`.
- `src/api/types.rs`
  Defines OpenAI-compatible request and response payloads used by handlers.
- `src/domain/mod.rs`
  Re-exports core internal types.
- `src/domain/request.rs`
  Defines the unified internal request representation.
- `src/domain/response.rs`
  Defines the unified internal response and streaming event representation.
- `src/models.rs`
  Stores model registry records and lookup helpers.
- `src/router.rs`
  Resolves a public model alias into a provider route target.
- `src/providers/mod.rs`
  Defines the provider adapter trait and shared provider utilities.
- `src/providers/openai.rs`
  OpenAI adapter implementation.
- `src/providers/anthropic.rs`
  Anthropic adapter implementation.
- `src/providers/gemini.rs`
  Gemini adapter implementation.
- `src/error.rs`
  Defines unified API errors and upstream error mapping.
- `src/sse.rs`
  Converts internal streaming events into OpenAI-style SSE chunks.
- `tests/models_api.rs`
  Integration tests for `GET /v1/models`.
- `tests/chat_api.rs`
  Integration tests for `POST /v1/chat/completions`.
- `tests/responses_api.rs`
  Integration tests for `POST /v1/responses`.
- `tests/common/mod.rs`
  Shared test helpers and mock upstream server setup.

## Configuration

The service is configured from environment variables. The exact variable naming
can be finalized in implementation, but the configuration model must support:

- Bind address and port
- Request timeout
- OpenAI API key
- Anthropic API key
- Gemini API key
- Model registry entries

Each model registry entry defines:

- Public model name exposed by `/v1/models`
- Provider kind: `openai`, `anthropic`, or `gemini`
- Upstream provider model name
- Optional capability flags, such as whether streaming is enabled

Example logical mapping:

- `gpt-4.1` -> provider `openai`, upstream model `gpt-4.1`
- `claude-sonnet-4` -> provider `anthropic`, upstream model
  `claude-sonnet-4-20250514`
- `gemini-2.5-pro` -> provider `gemini`, upstream model `gemini-2.5-pro`

The model registry is authoritative for routing. Requests for unknown public
model names must fail before any upstream call is attempted.

## API Behavior

### `GET /v1/models`

Returns the configured model registry as an OpenAI-style model list. Each entry
contains the public model identifier, owner metadata, and availability fields
needed by common SDKs. The response is generated locally and does not fan out to
providers at request time.

### `POST /v1/chat/completions`

Accepts an OpenAI-compatible chat completions payload. For the MVP, the unified
feature set includes:

- `model`
- `messages`
- `temperature`
- `max_tokens`
- `stream`

Messages must support at least `system`, `user`, and `assistant` roles with text
content. If a request uses fields outside the supported MVP contract, the API
must return a clear validation error rather than silently dropping behavior.

### `POST /v1/responses`

Accepts an OpenAI-style responses payload using the same internal routing and
provider execution path as chat completions. The responses API may normalize its
input into the same internal request form, but it should preserve response shape
expected by OpenAI-compatible clients.

To keep the MVP bounded, only text-generation paths need to be supported. Tool
calling, image generation, file references, and multimodal inputs remain out of
scope unless the internal request type can support them without introducing
provider-specific branching into the API layer.

## Unified Internal Model

The service needs one internal request model shared by chat completions and
responses. It should include:

- Public model name
- Resolved provider target
- Ordered message list
- Sampling options
- Maximum output tokens
- Stream flag
- Request metadata, including request ID

The internal response model should include:

- Provider-independent output text content
- Finish reason
- Usage metrics when available
- Provider metadata for logging

For streaming, define internal event variants that can represent:

- Stream start
- Delta text chunk
- Usage metadata if provided at end of stream
- Stream completion
- Stream error

This internal event model is what `src/sse.rs` converts into OpenAI-style SSE
chunks.

## Provider Adapters

Each provider adapter implements the same high-level contract:

1. Accept a resolved internal request
2. Build the upstream HTTP request
3. Execute it with configured auth and timeout
4. Convert the upstream response into unified internal output
5. Surface normalized errors

### OpenAI Adapter

This adapter is the simplest baseline because the external API is already close
to the public proxy contract. It should still flow through the same internal
model to avoid creating a special case.

### Anthropic Adapter

This adapter converts the internal message model into Anthropic's messages
format, handles system prompt placement, and maps streaming events into internal
delta events.

### Gemini Adapter

This adapter converts the internal request into Gemini's content format and
translates Gemini streaming or non-streaming responses back into the unified
model.

Provider adapters must own protocol translation. The API handlers must not know
about Anthropic request fields or Gemini content shapes.

## Routing

Routing is deterministic in the MVP. The `model` field from the client request
is looked up in the registry, and that record points to exactly one provider and
one upstream model. There is no dynamic latency-based or cost-based routing in
this version.

The routing layer is still separated from handlers so later versions can add:

- Fallback targets
- Weighted routing
- Region-specific routing
- Capability-aware selection

## Error Handling

All failures should return a stable OpenAI-style error envelope with a request
ID. Errors are grouped into:

- Authentication/configuration errors
- Validation errors
- Model not found
- Upstream provider HTTP errors
- Upstream timeout or network failures
- Streaming interruption failures

Internal logs should include:

- Request ID
- Public model name
- Resolved provider
- Upstream status code when available
- Error category

The API response should avoid leaking secrets or raw upstream payloads. Upstream
messages may be included in sanitized form when useful for debugging, but not in
a way that exposes credentials or internal stack traces.

## Streaming

Streaming must be exposed as OpenAI-style `text/event-stream`. The internal flow
is:

1. Handler validates the request and resolves model routing
2. Provider adapter yields internal stream events
3. SSE encoder maps events to OpenAI-compatible chunks
4. Completion is signaled with the terminal event expected by OpenAI clients

If an upstream provider disconnects mid-stream, the proxy should terminate the
downstream stream and emit a structured internal log entry. Whether an explicit
terminal error chunk is possible depends on how much of the stream has already
been emitted; the implementation should favor protocol correctness over trying to
repair a broken stream.

## Testing Strategy

Testing is part of the MVP, not follow-up work.

### Unit Tests

- Model registry parsing and lookup
- Routing behavior for known and unknown models
- Request conversion from API payload to internal model
- Response conversion from internal model to API payload
- Error mapping from provider failures to API errors
- SSE event encoding

### Integration Tests

Use mocked upstream HTTP servers to validate:

- `/v1/models` returns configured aliases
- Chat completions success against each provider adapter
- Responses success against each provider adapter
- Unknown model returns a validation or not-found error
- Upstream timeout returns normalized error
- Streaming path emits expected SSE sequence

Integration tests should not depend on live provider credentials.

## Non-Goals and Deferred Work

The design intentionally leaves out several OpenRouter-like capabilities:

- Per-request provider preferences
- Cost accounting and token price metadata
- Usage persistence
- Rate limiting
- API key issuance and tenant isolation
- Observability dashboards
- Automatic cross-provider retries

These can be added later because the MVP already establishes the critical seam:
OpenAI-compatible handlers, a unified internal model, and replaceable provider
adapters.

## Implementation Notes

Use a Rust web framework with good async and streaming support. `axum` is the
most pragmatic default for this codebase because it pairs cleanly with
`reqwest`, `tokio`, and SSE response handling.

Prefer explicit typed structs for public API payloads and internal domain types.
Avoid passing `serde_json::Value` through the core routing path except at the
very edges where provider APIs are genuinely irregular.

The first implementation should optimize for correctness and clear module
boundaries, not raw performance. Once the proxy works end-to-end with tests, it
will be straightforward to add metrics, pooling tweaks, and more advanced
routing behavior.
