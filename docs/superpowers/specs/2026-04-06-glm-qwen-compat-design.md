# GLM And Qwen Compatibility Layer Design

## Goal

Add GLM and Qwen Code as first-class upstream providers behind the existing
OpenAI-compatible proxy surface, while also supporting model-alias compatibility
for their public model names.

## Scope

This design includes:

- New provider kinds for `glm` and `qwen`
- Dedicated provider adapters under `src/providers/*`
- Config support for GLM and Qwen API keys and base URLs
- Model catalog entries that map GLM and Qwen-compatible public model names to
  upstream targets
- Non-streaming and streaming support through the existing proxy interfaces
- Tests for routing, non-streaming, and streaming behavior

This design does not include:

- New public HTTP endpoints specific to GLM or Qwen
- Provider-specific feature passthrough beyond the current unified request model
- Tool calling or other provider-native extensions
- New billing or quota semantics

## Recommended Approach

Continue using the current architecture: keep the public API surface unchanged,
normalize requests into the existing internal model, and implement provider
differences entirely inside dedicated adapters.

This is the right fit for the current codebase because the proxy already has
clear seams for provider configuration, route resolution, response
normalization, fallback routing, usage logging, and admin visibility. Adding
GLM and Qwen as explicit providers preserves those seams instead of forcing
special cases into the OpenAI adapter.

## Architecture

### Provider Additions

Extend `ProviderKind` with:

- `Glm`
- `Qwen`

Add provider implementations:

- `src/providers/glm.rs`
- `src/providers/qwen.rs`

These adapters follow the same contract as the existing providers:

1. Accept a `UnifiedRequest`
2. Build upstream request payloads
3. Execute the upstream call with provider-specific auth
4. Normalize non-streaming responses into `UnifiedResponse`
5. Normalize streaming events into the shared `EventStream`

### Compatibility Layer Boundary

The compatibility layer has two responsibilities only:

- Client-side compatibility:
  Accept public model names such as `glm-*` and `qwen-*` through the existing
  OpenAI-style endpoints.
- Upstream-side compatibility:
  Translate the current internal request model into GLM and Qwen upstream
  request formats, then translate responses back into the proxy's shared
  internal response format.

The compatibility layer must not introduce new top-level APIs or provider-
specific request shapes into the public handlers.

## Configuration

Add new config fields to `AppConfig`:

- `glm_api_key`
- `glm_base_url`
- `qwen_api_key`
- `qwen_base_url`

Add new environment variables:

- `GLM_API_KEY`
- `GLM_BASE_URL`
- `QWEN_API_KEY`
- `QWEN_BASE_URL`

Extend the model config schema so `provider` may be:

- `openai`
- `anthropic`
- `gemini`
- `glm`
- `qwen`

Example model catalog entries:

```json
{
  "public_name": "glm-4.5",
  "capabilities": {
    "chat_completions": true,
    "responses": true,
    "streaming": true
  },
  "pricing": {
    "currency": "USD",
    "input_per_million": 0.0,
    "output_per_million": 0.0
  },
  "targets": [
    {
      "provider": "glm",
      "upstream_name": "glm-4.5",
      "priority": 100,
      "capabilities": {
        "chat_completions": true,
        "responses": true,
        "streaming": true
      }
    }
  ]
}
```

```json
{
  "public_name": "qwen2.5-coder-32b-instruct",
  "capabilities": {
    "chat_completions": true,
    "responses": true,
    "streaming": true
  },
  "targets": [
    {
      "provider": "qwen",
      "upstream_name": "qwen2.5-coder-32b-instruct",
      "priority": 100,
      "capabilities": {
        "chat_completions": true,
        "responses": true,
        "streaming": true
      }
    }
  ]
}
```

## Request And Response Handling

### Request Side

The current unified request model remains the contract between handlers and
providers. No new public request fields are introduced in this phase.

Each adapter is responsible for:

- Mapping the unified message list into the upstream schema
- Translating system messages according to the upstream provider's expectations
- Setting `stream=true` when streaming is requested
- Choosing the correct upstream path for chat-style generation

### Response Side

Each adapter must normalize:

- Final text content
- Finish reason
- Usage fields when available
- Provider name and public model name

For streaming, each adapter must emit the shared internal event variants already
used by the proxy SSE encoder. The downstream output remains OpenAI-style SSE.

## Routing

No routing redesign is needed. The current route-plan mechanism already supports
multiple provider kinds and alias-based target selection. GLM and Qwen entries
simply become additional route targets in the existing model catalog.

This means:

- Alias compatibility is config-driven
- Fallback remains available if a model has multiple targets
- Admin APIs automatically surface GLM and Qwen entries once configured

## Error Handling

GLM and Qwen upstream errors must be mapped into the existing normalized error
envelope. The adapters should preserve useful upstream messages when possible,
but the public response still follows the proxy's stable error structure.

Streaming failures follow the same rule as the existing providers:

- If the upstream stream fails before downstream bytes are emitted, fallback may
  still be used by the route layer
- If downstream bytes were already emitted, terminate the stream and log the
  terminal failure

## Testing Strategy

### Unit Tests

- `ProviderKind` parsing recognizes `glm` and `qwen`
- Config parsing accepts GLM and Qwen model entries
- GLM request body mapping from `UnifiedRequest`
- Qwen request body mapping from `UnifiedRequest`

### Integration Tests

- Chat completion success through GLM adapter
- Chat completion success through Qwen adapter
- Streaming success through GLM adapter
- Streaming success through Qwen adapter
- Model alias routing resolves GLM names to `glm`
- Model alias routing resolves Qwen names to `qwen`

Tests should use mocked upstream servers, consistent with the rest of the
project.

## Success Criteria

This compatibility layer is complete when:

- GLM and Qwen are valid configured provider kinds
- Existing OpenAI-style endpoints can route to GLM and Qwen targets
- Non-streaming and streaming requests work through both adapters
- Model aliases for GLM and Qwen are resolved entirely through config
- Tests cover routing and adapter behavior
