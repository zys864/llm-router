# llm-router

An OpenRouter-style LLM proxy written in Rust.

## Features

- OpenAI-compatible `GET /v1/models`
- OpenAI-compatible `POST /v1/chat/completions`
- OpenAI-compatible `POST /v1/responses`
- Config-driven model aliasing
- Deterministic routing to OpenAI, Anthropic, and Gemini
- Fallback routing across multiple upstream targets
- Optional proxy bearer auth and request quotas
- Append-only usage logging to JSONL
- Unified JSON error envelopes
- Real upstream streaming normalized into OpenAI-style SSE output
- Unit and integration coverage with mocked upstream providers

## Quick Start

Copy the example environment file and adjust the keys and model mappings:

```bash
cp .env.example .env
```

Run the service:

```bash
cargo run
```

The service binds to `127.0.0.1:3000` by default.

## Configuration

Environment variables:

- `BIND_ADDR`
- `REQUEST_TIMEOUT_SECS`
- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`
- `GEMINI_API_KEY`
- `OPENAI_BASE_URL`
- `ANTHROPIC_BASE_URL`
- `GEMINI_BASE_URL`
- `MODEL_MAPPINGS`
- `MODEL_CONFIG_PATH`
- `PROXY_API_KEYS_PATH`
- `USAGE_LOG_PATH`

`MODEL_MAPPINGS` is a comma-separated list in this format:

```text
public-model=provider:upstream-model
```

Example:

```text
gpt-4.1=openai:gpt-4.1,claude-sonnet-4=anthropic:claude-sonnet-4-20250514,gemini-2.5-pro=gemini:gemini-2.5-pro
```

For richer routing, auth, and quota setup, use the example files in
`examples/model-config.json` and `examples/proxy-keys.json`.

## Example Requests

List models:

```bash
curl http://127.0.0.1:3000/v1/models
```

Chat completions:

```bash
curl -X POST http://127.0.0.1:3000/v1/chat/completions \
  -H 'content-type: application/json' \
  -d '{
    "model": "gpt-4.1",
    "messages": [
      { "role": "user", "content": "hello" }
    ]
  }'
```

Responses API:

```bash
curl -X POST http://127.0.0.1:3000/v1/responses \
  -H 'content-type: application/json' \
  -d '{
    "model": "gpt-4.1",
    "input": "hello"
  }'
```

Streaming chat completions:

```bash
curl -N -X POST http://127.0.0.1:3000/v1/chat/completions \
  -H 'content-type: application/json' \
  -d '{
    "model": "gpt-4.1",
    "messages": [
      { "role": "user", "content": "hello" }
    ],
    "stream": true
  }'
```

## Notes

- Streaming is normalized into OpenAI-style SSE chunks.
- `MODEL_CONFIG_PATH` enables multi-target model definitions and fallback order.
- `PROXY_API_KEYS_PATH` enables bearer auth and request quotas.
- `USAGE_LOG_PATH` appends one terminal JSONL record per request.
- Requests for unknown models fail before any upstream call is attempted.
