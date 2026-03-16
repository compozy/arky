//! Cross-provider token usage normalization and aggregation.

mod aggregator;
mod cost;
mod extractor;

pub use crate::{
    aggregator::UsageAggregator,
    cost::{
        ModelCost,
        compute_estimated_cost,
    },
    extractor::{
        ProviderMetadata,
        ProviderMetadataExtractor,
    },
};

use arky_protocol::{
    InputTokenDetails,
    OutputTokenDetails,
    Usage,
};
use serde::{
    Deserialize,
    Serialize,
};

/// Normalized usage data with cache and reasoning breakdowns.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NormalizedUsage {
    /// Prompt-side input tokens.
    pub input_tokens: u64,
    /// Completion-side output tokens.
    pub output_tokens: u64,
    /// Cached input tokens reused by the provider.
    pub cached_input_tokens: u64,
    /// Reasoning tokens when the provider reports them separately.
    pub reasoning_tokens: u64,
    /// Input-side breakdown.
    #[serde(default)]
    pub input_details: InputTokenDetails,
    /// Output-side breakdown.
    #[serde(default)]
    pub output_details: OutputTokenDetails,
    /// Estimated or provider-reported USD cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// Total runtime in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<f64>,
}

impl NormalizedUsage {
    /// Converts protocol usage into normalized usage.
    #[must_use]
    pub fn from_protocol_usage(usage: &Usage) -> Self {
        let input_details = usage.input_details.clone().unwrap_or_default();
        let output_details = usage.output_details.clone().unwrap_or_default();

        Self {
            input_tokens: usage.input_tokens.unwrap_or_default(),
            output_tokens: usage.output_tokens.unwrap_or_default(),
            cached_input_tokens: input_details.cache_read.unwrap_or_default()
                + input_details.cache_write.unwrap_or_default(),
            reasoning_tokens: output_details.reasoning.unwrap_or_default(),
            input_details,
            output_details,
            cost_usd: usage.cost_usd,
            duration_ms: usage.duration_ms,
        }
    }

    /// Projects normalized usage back to protocol usage.
    #[must_use]
    pub fn to_protocol_usage(&self) -> Usage {
        Usage {
            input_tokens: Some(self.input_tokens),
            output_tokens: Some(self.output_tokens),
            total_tokens: Some(self.input_tokens + self.output_tokens),
            input_details: Some(self.input_details.clone()),
            output_details: Some(self.output_details.clone()),
            cost_usd: self.cost_usd,
            duration_ms: self.duration_ms,
        }
    }

    /// Merges another usage snapshot into this one.
    pub fn merge(&mut self, other: &Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(other.cached_input_tokens);
        self.reasoning_tokens =
            self.reasoning_tokens.saturating_add(other.reasoning_tokens);
        merge_input_details(&mut self.input_details, &other.input_details);
        merge_output_details(&mut self.output_details, &other.output_details);
        self.cost_usd = sum_option_f64(self.cost_usd, other.cost_usd);
        self.duration_ms = sum_option_f64(self.duration_ms, other.duration_ms);
    }
}

const fn merge_input_details(target: &mut InputTokenDetails, other: &InputTokenDetails) {
    target.cache_read = sum_option_u64(target.cache_read, other.cache_read);
    target.cache_write = sum_option_u64(target.cache_write, other.cache_write);
    target.no_cache = sum_option_u64(target.no_cache, other.no_cache);
}

const fn merge_output_details(
    target: &mut OutputTokenDetails,
    other: &OutputTokenDetails,
) {
    target.text = sum_option_u64(target.text, other.text);
    target.reasoning = sum_option_u64(target.reasoning, other.reasoning);
}

const fn sum_option_u64(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.saturating_add(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn sum_option_f64(left: Option<f64>, right: Option<f64>) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left + right),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use crate::{
        ModelCost,
        NormalizedUsage,
        ProviderMetadata,
        ProviderMetadataExtractor,
        UsageAggregator,
    };

    use arky_protocol::{
        InputTokenDetails,
        OutputTokenDetails,
        Usage,
    };

    struct StaticExtractor;

    impl ProviderMetadataExtractor for StaticExtractor {
        fn extract_metadata(&self, raw: &serde_json::Value) -> ProviderMetadata {
            ProviderMetadata {
                session_id: raw
                    .get("session_id")
                    .and_then(serde_json::Value::as_str)
                    .map(ToOwned::to_owned),
                cost_usd: raw.get("cost_usd").and_then(serde_json::Value::as_f64),
                duration_ms: raw.get("duration_ms").and_then(serde_json::Value::as_f64),
                raw_usage: raw.get("usage").cloned(),
                warnings: vec!["fixture".to_owned()],
            }
        }
    }

    #[test]
    fn normalized_usage_should_accumulate_across_turns() {
        let base = Usage {
            input_tokens: Some(10),
            output_tokens: Some(4),
            total_tokens: Some(14),
            input_details: Some(InputTokenDetails {
                cache_read: Some(3),
                cache_write: Some(2),
                no_cache: Some(5),
            }),
            output_details: Some(OutputTokenDetails {
                text: Some(4),
                reasoning: Some(1),
            }),
            cost_usd: Some(0.2),
            duration_ms: Some(100.0),
        };
        let mut usage = NormalizedUsage::from_protocol_usage(&base);
        usage.merge(&NormalizedUsage::from_protocol_usage(&base));

        assert_eq!(usage.input_tokens, 20);
        assert_eq!(usage.output_tokens, 8);
        assert_eq!(usage.cached_input_tokens, 10);
        assert_eq!(usage.reasoning_tokens, 2);
    }

    #[test]
    fn usage_aggregator_should_merge_session_totals() {
        let usage = NormalizedUsage {
            input_tokens: 12,
            output_tokens: 8,
            cached_input_tokens: 4,
            reasoning_tokens: 2,
            input_details: InputTokenDetails::default(),
            output_details: OutputTokenDetails::default(),
            cost_usd: Some(0.1),
            duration_ms: Some(50.0),
        };
        let mut left = UsageAggregator::new();
        left.accumulate_turn(&usage);
        let mut right = UsageAggregator::new();
        right.accumulate_chunk(&usage);

        left.merge(&right);

        assert_eq!(left.session_total().input_tokens, 24);
        assert_eq!(left.session_total().output_tokens, 16);
    }

    #[test]
    fn model_cost_should_compute_common_families() {
        let usage = NormalizedUsage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            cached_input_tokens: 0,
            reasoning_tokens: 0,
            input_details: InputTokenDetails::default(),
            output_details: OutputTokenDetails::default(),
            cost_usd: None,
            duration_ms: None,
        };

        let claude_cost = ModelCost::compute_estimated_cost("claude-sonnet-4", &usage)
            .expect("claude pricing should resolve");
        let gpt_cost = ModelCost::compute_estimated_cost("gpt-4o", &usage)
            .expect("gpt pricing should resolve");

        assert_eq!(claude_cost > 0.0, true);
        assert_eq!(gpt_cost > 0.0, true);
        assert_eq!((claude_cost - gpt_cost).abs() > f64::EPSILON, true);
    }

    #[test]
    fn metadata_extractor_should_normalize_fields() {
        let metadata = StaticExtractor.extract_metadata(&json!({
            "session_id": "session-1",
            "cost_usd": 1.25,
            "duration_ms": 320.0,
            "usage": { "input_tokens": 10 }
        }));

        assert_eq!(metadata.session_id.as_deref(), Some("session-1"));
        assert_eq!(metadata.cost_usd, Some(1.25));
        assert_eq!(metadata.duration_ms, Some(320.0));
        assert_eq!(metadata.raw_usage, Some(json!({ "input_tokens": 10 })));
    }
}
