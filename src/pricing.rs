use serde::{Deserialize, Serialize};

use crate::config::ModelPricing;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CostEstimate {
    pub currency: String,
    pub amount: f64,
}

pub fn estimate_cost(
    pricing: &ModelPricing,
    input_tokens: Option<u32>,
    output_tokens: Option<u32>,
) -> CostEstimate {
    let input_cost =
        input_tokens.unwrap_or_default() as f64 / 1_000_000.0 * pricing.input_per_million;
    let output_cost =
        output_tokens.unwrap_or_default() as f64 / 1_000_000.0 * pricing.output_per_million;

    CostEstimate {
        currency: pricing.currency.clone(),
        amount: input_cost + output_cost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModelPricing;

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
}
