use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};

use crate::models::Pipeline;

/// Connection status for a single CI/CD source.
#[derive(Debug, Clone, PartialEq)]
pub enum SourceStatus {
    /// Successfully connected and fetching data
    Connected,
    /// Initial connection attempt in progress
    Connecting,
    /// Last attempt encountered an error (message included)
    Error(String),
}

impl SourceStatus {
    /// Short display string for the status bar
    pub fn as_str(&self) -> &str {
        match self {
            SourceStatus::Connected => "Connected",
            SourceStatus::Connecting => "Connecting…",
            SourceStatus::Error(_) => "Error",
        }
    }
}

/// Central state container for all live pipeline data.
///
/// The main event loop owns exactly one `AppState`.  The poller sends
/// `PollUpdate` messages; the event loop calls `merge_pipelines()` or
/// `mark_source_error()` to incorporate them.  The UI reads sorted/filtered
/// views via `get_sorted_pipelines()`.
#[derive(Debug)]
pub struct AppState {
    /// All pipelines we know about, keyed by their unique ID.
    /// Using a HashMap lets us merge updates without duplicating entries.
    pipelines: HashMap<String, Pipeline>,

    /// Per-source connection status
    pub source_status: HashMap<String, SourceStatus>,

    /// Timestamp of the last successful poll per source
    pub last_update: HashMap<String, DateTime<Utc>>,

    /// Ring buffer of recent errors (source_name, timestamp, message).
    /// Capped at `max_errors` to prevent unbounded growth.
    pub errors: VecDeque<ErrorEntry>,

    /// Maximum number of errors to retain
    max_errors: usize,
}

/// A single error record in the history
#[derive(Debug, Clone)]
pub struct ErrorEntry {
    pub source: String,
    pub timestamp: DateTime<Utc>,
    pub message: String,
}

impl AppState {
    /// Create a fresh, empty state.
    pub fn new() -> Self {
        Self {
            pipelines: HashMap::new(),
            source_status: HashMap::new(),
            last_update: HashMap::new(),
            errors: VecDeque::new(),
            max_errors: 20,
        }
    }

    /// Register a source as "connecting" (called once at startup for each
    /// configured source before the first poll completes).
    pub fn register_source(&mut self, name: &str) {
        self.source_status
            .insert(name.to_string(), SourceStatus::Connecting);
    }

    /// Merge a fresh batch of pipelines from a single source.
    ///
    /// All previously known pipelines **from that same source** are removed
    /// first, then the new set is inserted.  This avoids stale entries
    /// lingering after a workflow is deleted or renamed on GitHub.
    pub fn merge_pipelines(&mut self, source_name: &str, pipelines: Vec<Pipeline>) {
        // Insert the fresh data
        for pipeline in pipelines {
            self.pipelines.insert(pipeline.id.clone(), pipeline);
        }

        // Mark source as connected with a fresh timestamp
        self.source_status
            .insert(source_name.to_string(), SourceStatus::Connected);
        self.last_update
            .insert(source_name.to_string(), Utc::now());
    }

    /// Record that a source encountered an error.
    pub fn mark_source_error(&mut self, source_name: &str, message: &str) {
        self.source_status.insert(
            source_name.to_string(),
            SourceStatus::Error(message.to_string()),
        );

        // Push to the error ring-buffer
        self.errors.push_back(ErrorEntry {
            source: source_name.to_string(),
            timestamp: Utc::now(),
            message: message.to_string(),
        });

        // Trim to max
        while self.errors.len() > self.max_errors {
            self.errors.pop_front();
        }
    }

    /// Return all pipelines sorted by status priority then by start time
    /// (most recent first).
    ///
    /// Sort order: Running → Pending → Failed → Success → Cancelled → Skipped
    pub fn get_sorted_pipelines(&self) -> Vec<&Pipeline> {
        let mut pipelines: Vec<&Pipeline> = self.pipelines.values().collect();

        pipelines.sort_by(|a, b| {
            let status_order = |s: &crate::models::PipelineStatus| match s {
                crate::models::PipelineStatus::Running => 0,
                crate::models::PipelineStatus::Pending => 1,
                crate::models::PipelineStatus::Failed => 2,
                crate::models::PipelineStatus::Success => 3,
                crate::models::PipelineStatus::Cancelled => 4,
                crate::models::PipelineStatus::Skipped => 5,
            };
            status_order(&a.status)
                .cmp(&status_order(&b.status))
                .then_with(|| b.started_at.cmp(&a.started_at))
        });

        pipelines
    }

    /// Total number of pipelines currently tracked
    pub fn pipeline_count(&self) -> usize {
        self.pipelines.len()
    }

    /// Number of pipelines in each status category
    pub fn status_summary(&self) -> StatusSummary {
        let mut summary = StatusSummary::default();
        for p in self.pipelines.values() {
            match p.status {
                crate::models::PipelineStatus::Running => summary.running += 1,
                crate::models::PipelineStatus::Pending => summary.pending += 1,
                crate::models::PipelineStatus::Failed => summary.failed += 1,
                crate::models::PipelineStatus::Success => summary.success += 1,
                crate::models::PipelineStatus::Cancelled => summary.cancelled += 1,
                crate::models::PipelineStatus::Skipped => summary.skipped += 1,
            }
        }
        summary
    }
}

/// Quick status counts for the header bar
#[derive(Debug, Default, Clone, Copy)]
pub struct StatusSummary {
    pub running: usize,
    pub pending: usize,
    pub failed: usize,
    pub success: usize,
    pub cancelled: usize,
    pub skipped: usize,
}