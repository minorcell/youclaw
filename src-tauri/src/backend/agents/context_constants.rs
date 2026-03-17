//! Shared constants for chat context assembly/compaction.

/// Character budget for merged session summaries.
pub(crate) const SUMMARY_CHAR_LIMIT: usize = 16_000;
/// Marker for persisted compressed summary.
pub(crate) const SUMMARY_MARKER: &str = "[previous-summary]";
/// Marker for ephemeral in-memory step summary.
pub(crate) const STEP_SUMMARY_MARKER: &str = "[step-summary]";
