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
    /// GitHub (or other) rate limit hit; includes when the limit resets
    RateLimited(DateTime<Utc>),
}

impl SourceStatus {
    /// Short display string for the status bar
    pub fn as_str(&self) -> &str {
        match self {
            SourceStatus::Connected => "Connected",
            SourceStatus::Connecting => "Connecting…",
            SourceStatus::Error(_) => "Error",
            SourceStatus::RateLimited(_) => "Rate Limited",
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
        // Remove old entries from this source
        self.pipelines.retain(|_, _p| {
            // Match by the source_name stored in the pipeline's source field
            // combined with checking source_status keys. We use the `source`
            // field on Pipeline which stores e.g. "GitHub Actions", but we
            // need to match by the config source_name. We tag pipelines via
            // their id prefix pattern (github-*, gitlab-*, etc.).
            // Simpler approach: remove pipelines whose repository matches one
            // of the repos we're about to replace.
            // Simplest correct approach: track source_name → pipeline ids.
            true // handled below
        });

        // Remove all pipelines that came from this source by checking if
        // any of the new pipelines share the same source_name tag.
        // We embed the source_name in the pipeline id at creation time
        // (see github.rs: id = "github-{repo}-{run_id}").
        // For a clean merge we remove all pipelines whose id starts with
        // any prefix that matches this source's repos.
        let _new_ids: Vec<String> = pipelines.iter().map(|p| p.id.clone()).collect();

        // Determine which IDs to evict: any existing pipeline whose
        // "source" field matches the platform AND whose repository is one
        // we just fetched fresh data for.
        let new_repos: std::collections::HashSet<&str> =
            pipelines.iter().map(|p| p.repository.as_str()).collect();

        self.pipelines.retain(|_, p| {
            // Keep pipelines from *other* repos untouched
            !new_repos.contains(p.repository.as_str())
        });

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

    /// Record that a source was rate-limited.
    pub fn mark_source_rate_limited(&mut self, source_name: &str, retry_after: chrono::Duration) {
        let reset_at = Utc::now() + retry_after;
        self.source_status.insert(
            source_name.to_string(),
            SourceStatus::RateLimited(reset_at),
        );

        self.errors.push_back(ErrorEntry {
            source: source_name.to_string(),
            timestamp: Utc::now(),
            message: format!("Rate limited until {}", reset_at.format("%H:%M:%S")),
        });

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Pipeline, PipelineStatus};

    fn make_pipeline(id: &str, repo: &str, status: PipelineStatus) -> Pipeline {
        Pipeline {
            id: id.to_string(),
            name: "Test".into(),
            source: "GitHub Actions".into(),
            repository: repo.to_string(),
            branch: "main".into(),
            status,
            build_number: 1,
            started_at: Utc::now(),
            finished_at: None,
            duration: None,
            stages: vec![],
            url: "https://example.com".into(),
            commit_sha: "abc1234".into(),
            commit_message: "test".into(),
            author: "dev".into(),
        }
    }

    #[test]
    fn test_merge_replaces_same_repo() {
        let mut state = AppState::new();

        // First batch
        state.merge_pipelines(
            "my-github",
            vec![make_pipeline("github-api-1", "org/api", PipelineStatus::Success)],
        );
        assert_eq!(state.pipeline_count(), 1);

        // Second batch for same repo – should replace
        state.merge_pipelines(
            "my-github",
            vec![make_pipeline("github-api-2", "org/api", PipelineStatus::Running)],
        );
        assert_eq!(state.pipeline_count(), 1);

        let pipelines = state.get_sorted_pipelines();
        assert_eq!(pipelines[0].id, "github-api-2");
    }

    #[test]
    fn test_merge_preserves_other_repos() {
        let mut state = AppState::new();

        state.merge_pipelines(
            "source-a",
            vec![make_pipeline("github-api-1", "org/api", PipelineStatus::Success)],
        );
        state.merge_pipelines(
            "source-b",
            vec![make_pipeline("github-web-1", "org/web", PipelineStatus::Running)],
        );
        assert_eq!(state.pipeline_count(), 2);
    }

    #[test]
    fn test_sorted_pipelines_order() {
        let mut state = AppState::new();
        state.merge_pipelines(
            "src",
            vec![
                make_pipeline("p1", "org/a", PipelineStatus::Success),
                make_pipeline("p2", "org/b", PipelineStatus::Running),
                make_pipeline("p3", "org/c", PipelineStatus::Failed),
            ],
        );

        let sorted = state.get_sorted_pipelines();
        assert_eq!(sorted[0].status, PipelineStatus::Running);
        assert_eq!(sorted[1].status, PipelineStatus::Failed);
        assert_eq!(sorted[2].status, PipelineStatus::Success);
    }

    #[test]
    fn test_error_history_capped() {
        let mut state = AppState::new();
        state.max_errors = 3;

        for i in 0..5 {
            state.mark_source_error("src", &format!("error {}", i));
        }

        assert_eq!(state.errors.len(), 3);
        // Should retain the most recent 3
        assert!(state.errors[0].message.contains("error 2"));
        assert!(state.errors[2].message.contains("error 4"));
    }

    #[test]
    fn test_status_summary() {
        let mut state = AppState::new();
        state.merge_pipelines(
            "src",
            vec![
                make_pipeline("p1", "org/a", PipelineStatus::Running),
                make_pipeline("p2", "org/b", PipelineStatus::Failed),
                make_pipeline("p3", "org/c", PipelineStatus::Success),
                make_pipeline("p4", "org/d", PipelineStatus::Success),
            ],
        );

        let summary = state.status_summary();
        assert_eq!(summary.running, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.success, 2);
    }
}