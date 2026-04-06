# llm-router

An OpenRouter-style LLM proxy written in Rust.

## Features

- OpenAI-compatible `GET /v1/models`
- OpenAI-compatible `POST /v1/chat/completions`
- OpenAI-compatible `POST /v1/responses`
- Config-driven model aliasing
- Deterministic routing to OpenAI, Anthropic, and Gemini
- Unified JSON error envelopes
- OpenAI-style SSE output for streaming requests
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

`MODEL_MAPPINGS` is a comma-separated list in this format:

```text
public-model=provider:upstream-model
```

Example:

```text
gpt-4.1=openai:gpt-4.1,claude-sonnet-4=anthropic:claude-sonnet-4-20250514,gemini-2.5-pro=gemini:gemini-2.5-pro
```

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
- Provider streaming is implemented as normalized proxy output for the MVP.
- Requests for unknown models fail before any upstream call is attempted.
