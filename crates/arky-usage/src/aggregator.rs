//! Usage aggregation across chunks, turns, and sessions.

use crate::NormalizedUsage;

/// Accumulates normalized usage over time.
#[derive(Debug, Clone, Default)]
pub struct UsageAggregator {
    current_turn: Option<NormalizedUsage>,
    session_total: NormalizedUsage,
}

impl UsageAggregator {
    /// Creates an empty aggregator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds usage from one streamed chunk to the current turn.
    pub fn accumulate_chunk(&mut self, chunk: &NormalizedUsage) {
        let current = self
            .current_turn
            .get_or_insert_with(NormalizedUsage::default);
        current.merge(chunk);
        self.session_total.merge(chunk);
    }

    /// Adds one completed turn's usage and starts a new current turn snapshot.
    pub fn accumulate_turn(&mut self, turn: &NormalizedUsage) {
        self.current_turn = Some(turn.clone());
        self.session_total.merge(turn);
    }

    /// Returns the current in-flight turn snapshot when available.
    #[must_use]
    pub const fn current_turn(&self) -> Option<&NormalizedUsage> {
        self.current_turn.as_ref()
    }

    /// Returns the accumulated session totals.
    #[must_use]
    pub const fn session_total(&self) -> &NormalizedUsage {
        &self.session_total
    }

    /// Merges another aggregator into this one.
    pub fn merge(&mut self, other: &Self) {
        if let Some(turn) = &other.current_turn {
            self.current_turn
                .get_or_insert_with(NormalizedUsage::default)
                .merge(turn);
        }
        self.session_total.merge(&other.session_total);
    }
}
