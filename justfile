set dotenv-load := true

run:
    ENABLE_PROVIDER_DEFAULT_AUTH_FALLBACK=true cargo run

run-openai:
    if [ -z "${OPENAI_API_KEY:-}" ]; then echo "OPENAI_API_KEY is required"; exit 1; fi
    cargo run
