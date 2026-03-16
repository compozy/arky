//! Estimated per-model cost helpers.

use crate::NormalizedUsage;

const PRICING: &[ModelCost] = &[
    ModelCost {
        family: "claude-3-5-sonnet",
        input_per_million: 3.0,
        output_per_million: 15.0,
        cached_input_per_million: 0.3,
        reasoning_per_million: 15.0,
    },
    ModelCost {
        family: "claude-3.5-sonnet",
        input_per_million: 3.0,
        output_per_million: 15.0,
        cached_input_per_million: 0.3,
        reasoning_per_million: 15.0,
    },
    ModelCost {
        family: "claude-3.7-sonnet",
        input_per_million: 3.0,
        output_per_million: 15.0,
        cached_input_per_million: 0.3,
        reasoning_per_million: 15.0,
    },
    ModelCost {
        family: "claude-sonnet-4",
        input_per_million: 3.0,
        output_per_million: 15.0,
        cached_input_per_million: 0.3,
        reasoning_per_million: 15.0,
    },
    ModelCost {
        family: "claude-opus",
        input_per_million: 15.0,
        output_per_million: 75.0,
        cached_input_per_million: 1.5,
        reasoning_per_million: 75.0,
    },
    ModelCost {
        family: "gpt-4o",
        input_per_million: 5.0,
        output_per_million: 15.0,
        cached_input_per_million: 2.5,
        reasoning_per_million: 15.0,
    },
    ModelCost {
        family: "gpt-4o-mini",
        input_per_million: 0.15,
        output_per_million: 0.6,
        cached_input_per_million: 0.075,
        reasoning_per_million: 0.6,
    },
    ModelCost {
        family: "gpt-4.1",
        input_per_million: 2.0,
        output_per_million: 8.0,
        cached_input_per_million: 0.5,
        reasoning_per_million: 8.0,
    },
    ModelCost {
        family: "gpt-5",
        input_per_million: 1.25,
        output_per_million: 10.0,
        cached_input_per_million: 0.125,
        reasoning_per_million: 10.0,
    },
    ModelCost {
        family: "gpt-5.1-codex",
        input_per_million: 1.25,
        output_per_million: 10.0,
        cached_input_per_million: 0.125,
        reasoning_per_million: 10.0,
    },
    ModelCost {
        family: "codex-mini-latest",
        input_per_million: 1.5,
        output_per_million: 6.0,
        cached_input_per_million: 0.15,
        reasoning_per_million: 6.0,
    },
];

/// Per-million-token pricing metadata for one model family.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelCost {
    /// Prefix or family key used for lookup.
    pub family: &'static str,
    /// USD price per million input tokens.
    pub input_per_million: f64,
    /// USD price per million output tokens.
    pub output_per_million: f64,
    /// USD price per million cached input tokens, when distinct.
    pub cached_input_per_million: f64,
    /// USD price per million reasoning tokens, when distinct from output.
    pub reasoning_per_million: f64,
}

impl ModelCost {
    /// Estimates usage cost for one model identifier and usage payload.
    #[must_use]
    pub fn compute_estimated_cost(
        model_id: &str,
        usage: &NormalizedUsage,
    ) -> Option<f64> {
        let pricing = pricing_for_model(model_id)?;
        let input_tokens = f64_from_u64(billable_input_tokens(usage))? / 1_000_000_f64;
        let output_tokens = f64_from_u64(billable_output_tokens(usage))? / 1_000_000_f64;
        let cached_tokens = f64_from_u64(usage.cached_input_tokens)? / 1_000_000_f64;
        let reasoning_tokens = f64_from_u64(usage.reasoning_tokens)? / 1_000_000_f64;

        let cost = reasoning_tokens.mul_add(
            pricing.reasoning_per_million,
            cached_tokens.mul_add(
                pricing.cached_input_per_million,
                input_tokens.mul_add(
                    pricing.input_per_million,
                    output_tokens * pricing.output_per_million,
                ),
            ),
        );

        Some(cost)
    }
}

/// Estimates usage cost for one model identifier and usage payload.
#[must_use]
pub fn compute_estimated_cost(model_id: &str, usage: &NormalizedUsage) -> Option<f64> {
    ModelCost::compute_estimated_cost(model_id, usage)
}

fn f64_from_u64(value: u64) -> Option<f64> {
    value.to_string().parse::<f64>().ok()
}

fn billable_input_tokens(usage: &NormalizedUsage) -> u64 {
    usage
        .input_details
        .no_cache
        .unwrap_or_else(|| usage.input_tokens.saturating_sub(usage.cached_input_tokens))
}

fn billable_output_tokens(usage: &NormalizedUsage) -> u64 {
    usage
        .output_details
        .text
        .unwrap_or_else(|| usage.output_tokens.saturating_sub(usage.reasoning_tokens))
}

fn pricing_for_model(model_id: &str) -> Option<ModelCost> {
    let normalized = model_id.trim().to_lowercase();
    PRICING
        .iter()
        .find(|pricing| normalized.contains(pricing.family))
        .copied()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::compute_estimated_cost;
    use crate::NormalizedUsage;
    use arky_protocol::{
        InputTokenDetails,
        OutputTokenDetails,
    };

    #[test]
    fn compute_estimated_cost_should_price_claude_3_5_sonnet_usage() {
        let usage = NormalizedUsage {
            input_tokens: 1_100_000,
            output_tokens: 520_000,
            cached_input_tokens: 100_000,
            reasoning_tokens: 20_000,
            input_details: InputTokenDetails {
                no_cache: Some(1_000_000),
                cache_read: Some(100_000),
                cache_write: None,
            },
            output_details: OutputTokenDetails {
                text: Some(500_000),
                reasoning: Some(20_000),
            },
            cost_usd: None,
            duration_ms: None,
        };

        let cost = compute_estimated_cost("claude-3.5-sonnet", &usage)
            .expect("claude pricing should resolve");

        assert_eq!((cost - 10.83).abs() < 0.000_001, true);
    }

    #[test]
    fn compute_estimated_cost_should_price_gpt_4o_usage() {
        let usage = NormalizedUsage {
            input_tokens: 1_500_000,
            output_tokens: 750_000,
            cached_input_tokens: 500_000,
            reasoning_tokens: 0,
            input_details: InputTokenDetails {
                no_cache: Some(1_000_000),
                cache_read: Some(500_000),
                cache_write: None,
            },
            output_details: OutputTokenDetails {
                text: Some(750_000),
                reasoning: None,
            },
            cost_usd: None,
            duration_ms: None,
        };

        let cost =
            compute_estimated_cost("gpt-4o", &usage).expect("gpt pricing should resolve");

        assert_eq!((cost - 17.5).abs() < 0.000_001, true);
    }

    #[test]
    fn compute_estimated_cost_should_return_none_for_unknown_models() {
        assert_eq!(
            compute_estimated_cost("mystery-model", &NormalizedUsage::default()),
            None
        );
    }
}
