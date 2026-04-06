# LLM Proxy Phase 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add pricing metadata, usage aggregation, quota recovery, and read-only admin APIs to the LLM proxy.

**Architecture:** Extend the existing JSONL usage log into a lightweight read path instead of introducing a new storage backend. Keep request handling append-only and fast, then layer pricing, aggregation, recovery, and admin APIs on top of the same log and model catalog.

**Tech Stack:** Rust, `axum`, `tokio`, `serde`, `serde_json`, `reqwest`, `tower`, `wiremock`

---

## File Structure

- Create: `src/pricing.rs`
- Create: `src/admin.rs`
- Create: `src/usage_aggregate.rs`
- Modify: `src/lib.rs`
- Modify: `src/config.rs`
- Modify: `src/models.rs`
- Modify: `src/quota.rs`
- Modify: `src/usage.rs`
- Modify: `src/api/mod.rs`
- Modify: `README.md`
- Modify: `.env.example`
- Modify: `examples/model-config.json`
- Modify: `tests/common/mod.rs`
- Create: `tests/admin_api.rs`
- Modify: `tests/logging_api.rs`

### Task 1: Add pricing metadata to model config and registry

**Files:**
- Modify: `src/config.rs`
- Modify: `src/models.rs`
- Modify: `examples/model-config.json`
- Test: `src/config.rs`

- [ ] **Step 1: Write the failing config pricing test**

```rust
#[test]
fn loads_model_pricing_from_json_file() {
    let config =
        AppConfig::from_test_paths("examples/model-config.json", None::<&str>, None::<&str>)
            .unwrap();

    let pricing = config.models[1].pricing.as_ref().unwrap();
    assert_eq!(pricing.currency, "USD");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test loads_model_pricing_from_json_file -q`
Expected: FAIL with missing pricing metadata types

- [ ] **Step 3: Implement pricing types in config and model registry**

```rust
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ModelPricing {
    pub currency: String,
    pub input_per_million: f64,
    pub output_per_million: f64,
}
```

- [ ] **Step 4: Run targeted test**

Run: `cargo test loads_model_pricing_from_json_file -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/models.rs examples/model-config.json
git commit -m "feat: add model pricing metadata"
```

### Task 2: Add pricing estimation helpers

**Files:**
- Create: `src/pricing.rs`
- Test: `src/pricing.rs`

- [ ] **Step 1: Write the failing pricing estimation test**

```rust
#[test]
fn estimates_cost_from_usage_tokens() {
    let pricing = ModelPricing {
        currency: "USD".into(),
        input_per_million: 2.0,
        output_per_million: 8.0,
    };

    let estimate = estimate_cost(&pricing, Some(1_000_000), Some(500_000));
    assert_eq!(estimate.currency, "USD");
    assert!(estimate.amount > 0.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test estimates_cost_from_usage_tokens -q`
Expected: FAIL with missing pricing helpers

- [ ] **Step 3: Implement pricing helpers**

```rust
pub fn estimate_cost(
    pricing: &ModelPricing,
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
) -> CostEstimate
```

- [ ] **Step 4: Run targeted test**

Run: `cargo test estimates_cost_from_usage_tokens -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/pricing.rs src/lib.rs
git commit -m "feat: add pricing estimation helpers"
```

### Task 3: Expand usage records with timestamp, tokens, and estimated cost

**Files:**
- Modify: `src/usage.rs`
- Modify: `src/api/chat_completions.rs`
- Modify: `src/api/responses.rs`
- Test: `tests/logging_api.rs`

- [ ] **Step 1: Write the failing logging field test**

```rust
#[tokio::test]
async fn writes_usage_record_with_cost_fields() {
    let (app, log_path) = usage_logging_app().await;
    let _ = post_json(app, "/v1/chat/completions", sample_chat_body("gpt-4.1")).await;

    let body = tokio::fs::read_to_string(log_path).await.unwrap();
    assert!(body.contains("\"timestamp\""));
    assert!(body.contains("\"estimated_cost\""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test writes_usage_record_with_cost_fields -q`
Expected: FAIL with missing fields on usage records

- [ ] **Step 3: Expand usage records and logging hooks**

```rust
pub struct UsageRecord {
    pub timestamp: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub estimated_cost: Option<CostEstimate>,
}
```

- [ ] **Step 4: Run targeted test**

Run: `cargo test writes_usage_record_with_cost_fields -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/usage.rs src/api/chat_completions.rs src/api/responses.rs tests/logging_api.rs
git commit -m "feat: add enriched usage records"
```

### Task 4: Add usage aggregation over JSONL logs

**Files:**
- Create: `src/usage_aggregate.rs`
- Modify: `src/usage.rs`
- Test: `src/usage_aggregate.rs`

- [ ] **Step 1: Write the failing aggregation test**

```rust
#[tokio::test]
async fn aggregates_usage_by_caller_and_model() {
    let path = write_usage_fixture().await;
    let summary = UsageAggregator::from_path(&path).await.unwrap().summarize().unwrap();

    assert_eq!(summary.total_requests, 2);
    assert_eq!(summary.by_caller["team-alpha"], 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test aggregates_usage_by_caller_and_model -q`
Expected: FAIL with missing aggregation layer

- [ ] **Step 3: Implement the aggregation layer**

```rust
pub struct UsageSummary {
    pub total_requests: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub by_caller: HashMap<String, usize>,
    pub by_model: HashMap<String, usize>,
    pub by_provider: HashMap<String, usize>,
    pub estimated_cost_total: f64,
}
```

- [ ] **Step 4: Run targeted test**

Run: `cargo test aggregates_usage_by_caller_and_model -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/usage_aggregate.rs src/usage.rs
git commit -m "feat: add usage aggregation"
```

### Task 5: Recover quota counters from usage log at startup

**Files:**
- Modify: `src/quota.rs`
- Modify: `src/lib.rs`
- Test: `tests/admin_api.rs`

- [ ] **Step 1: Write the failing quota recovery test**

```rust
#[tokio::test]
async fn recovers_used_quota_from_existing_usage_log() {
    let (app, _) = recovered_quota_app().await;

    let response = post_json_with_bearer(
        app,
        "/v1/chat/completions",
        "lr_live_alpha",
        sample_chat_body("gpt-4.1"),
    )
    .await;

    assert_eq!(response.status, 429);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test recovers_used_quota_from_existing_usage_log -q`
Expected: FAIL with no startup recovery

- [ ] **Step 3: Implement quota recovery**

```rust
pub async fn seed_usage(&self, recovered_counts: HashMap<String, u64>) -> Result<(), AppError>
```

- [ ] **Step 4: Run targeted test**

Run: `cargo test recovers_used_quota_from_existing_usage_log -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/quota.rs src/lib.rs tests/admin_api.rs tests/common/mod.rs
git commit -m "feat: recover quota from usage log"
```

### Task 6: Add read-only admin APIs

**Files:**
- Create: `src/admin.rs`
- Modify: `src/api/mod.rs`
- Create: `tests/admin_api.rs`

- [ ] **Step 1: Write the failing admin integration tests**

```rust
#[tokio::test]
async fn admin_models_returns_pricing_metadata() {
    let app = auth_enabled_admin_app().await;
    let response = get_with_bearer(app, "/admin/models", "lr_live_alpha").await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("\"pricing\""));
}

#[tokio::test]
async fn admin_usage_summary_returns_aggregates() {
    let app = auth_enabled_admin_app().await;
    let response = get_with_bearer(app, "/admin/usage/summary", "lr_live_alpha").await;

    assert_eq!(response.status, 200);
    assert!(response.body.contains("\"total_requests\""));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test admin_models_returns_pricing_metadata admin_usage_summary_returns_aggregates -q`
Expected: FAIL with missing admin routes

- [ ] **Step 3: Implement admin routes and response types**

```rust
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/admin/models", get(models))
        .route("/admin/callers", get(callers))
        .route("/admin/usage/summary", get(usage_summary))
        .with_state(state)
}
```

- [ ] **Step 4: Run targeted tests**

Run: `cargo test admin_models_returns_pricing_metadata admin_usage_summary_returns_aggregates -q`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/admin.rs src/api/mod.rs tests/admin_api.rs tests/common/mod.rs
git commit -m "feat: add admin read apis"
```

### Task 7: Update docs and final verification

**Files:**
- Modify: `README.md`
- Modify: `.env.example`
- Modify: `examples/model-config.json`

- [ ] **Step 1: Update docs**

```text
Document pricing metadata, admin routes, and quota recovery behavior.
```

- [ ] **Step 2: Run formatting**

Run: `cargo fmt --all`
Expected: command completes with no errors

- [ ] **Step 3: Run the full test suite**

Run: `cargo test`
Expected: PASS for unit and integration tests

- [ ] **Step 4: Run a smoke start**

Run: `cargo run`
Expected: service starts without panic and binds to configured address

- [ ] **Step 5: Commit**

```bash
git add README.md .env.example examples/model-config.json docs/superpowers/plans/2026-04-06-llm-proxy-phase-3.md
git commit -m "feat: add phase 3 admin and pricing capabilities"
```
