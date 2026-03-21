//! Shared constants for chat context assembly/compaction.

/// Marker for compressed session context injected into the prompt.
pub(crate) const SUMMARY_MARKER: &str = "[compressed-context]";
/// Marker for the runtime memory reflection hint injected before the latest user message.
pub(crate) const MEMORY_HINT_MARKER: &str = "<memory-hint>";
