# LLM Proxy Phase 3 Design

## Goal

Add a lightweight platform operations layer on top of the proxy by introducing
model pricing metadata, usage aggregation from JSONL logs, quota recovery on
startup, and read-only admin APIs for operators.

## Scope

This phase includes:

- Model pricing metadata in the model catalog
- Usage aggregation from the append-only JSONL usage log
- Quota counter recovery from historical usage records at startup
- Read-only admin APIs for models, callers, and usage summaries
- Updated tests and docs for the new operator-facing behavior

This phase does not include:

- Balance deduction or prepaid wallet logic
- Database-backed analytics storage
- Writable admin APIs
- Historical pagination beyond reading the existing JSONL log
- Token-accurate billing enforcement

## Recommended Approach

Use the current JSONL usage log as the system of record for lightweight
operations features instead of adding a database. The service should continue to
append one terminal record per request, but it should also gain the ability to
read and aggregate those records efficiently enough for small to medium local
deployments.

This keeps the architecture coherent: request handling remains fast and simple,
while operator APIs are served from a read path that scans or reuses the log.

## Architecture

### New Core Components

- `src/pricing.rs`
  Defines price metadata and cost estimation helpers.
- `src/admin.rs`
  Defines admin response types and read-only route wiring.
- `src/usage_aggregate.rs`
  Reads JSONL usage records and computes summaries.

Existing components extended:

- `src/config.rs`
  Loads optional per-model pricing metadata.
- `src/models.rs`
  Stores pricing metadata with model entries.
- `src/quota.rs`
  Supports seeding counters from recovered usage totals.
- `src/usage.rs`
  Includes token fields and estimated cost in usage records.
- `src/lib.rs`
  Loads recovered usage state during startup before accepting traffic.

## Configuration

### Model Pricing Metadata

Each model catalog entry may include a `pricing` object:

```json
{
  "public_name": "gpt-4.1",
  "capabilities": {
    "chat_completions": true,
    "responses": true,
    "streaming": true
  },
  "pricing": {
    "currency": "USD",
    "input_per_million": 2.0,
    "output_per_million": 8.0
  },
  "targets": [
    {
      "provider": "openai",
      "upstream_name": "gpt-4.1",
      "priority": 100
    }
  ]
}
```

Pricing metadata is advisory in this phase. It is used to estimate cost in
usage summaries and admin responses, not to block requests.

### Usage Recovery

If `USAGE_LOG_PATH` is configured and the file exists, startup should read the
log and reconstruct per-caller successful request counts before serving traffic.

Recovery rules:

- Only records with a caller ID affect recovered quotas
- Only `success` records increment recovered request counts
- Failed or rejected requests do not consume recovered quota

## Usage Aggregation

The proxy already appends one terminal record per request. This phase expands
the record schema to include:

- Input tokens when available
- Output tokens when available
- Estimated cost when model pricing metadata exists
- Timestamp

Aggregation should support:

- Totals by caller
- Totals by public model
- Totals by provider
- Totals by status
- Aggregate estimated cost

Implementation can scan the full JSONL file on demand for now. The read path is
acceptable for local deployments and avoids introducing a new storage system.

## Admin APIs

These routes should be added under `/admin` and protected by the same proxy
bearer auth layer. In this phase, any valid proxy key may read them.

### `GET /admin/models`

Returns the configured model catalog, including:

- Public model name
- Capabilities
- Target count
- Pricing metadata when configured

### `GET /admin/callers`

Returns configured callers and their current recovered-plus-live request usage:

- Caller ID
- Max requests
- Requests used
- Requests remaining

### `GET /admin/usage/summary`

Returns aggregate metrics derived from the usage log:

- Total request count
- Success count
- Failure count
- Requests by caller
- Requests by model
- Requests by provider
- Estimated cost totals

## Error Handling

Add one new internal error category for admin read failures when the usage log
cannot be scanned or parsed. Externally these still map to `server_error`.

Malformed JSONL lines should be skipped with internal logging instead of
bringing down the whole admin response or startup recovery process.

## Testing Strategy

### Unit Tests

- Pricing estimation from usage tokens
- Usage aggregation from multiple JSONL records
- Quota recovery from historical success records
- Model config parsing with pricing metadata

### Integration Tests

- Admin models endpoint returns pricing metadata
- Admin callers endpoint returns recovered and live quota usage
- Admin usage summary endpoint returns aggregated counters
- Startup recovery restores quota usage from a prewritten usage log

## Success Criteria

This phase is complete when:

- Model catalog entries may include pricing metadata
- Usage records include token and estimated-cost fields when available
- Quota counters recover from the existing usage log on startup
- Operator-facing read-only admin APIs return aggregated data
- Tests and docs cover the new behavior
