use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::stage::Stage;

/// Represents a CI/CD pipeline execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    /// Unique identifier for this pipeline run
    pub id: String,

    /// Human-readable name of the pipeline
    pub name: String,

    /// Source platform (e.g., "GitHub Actions", "GitLab CI", "Jenkins")
    pub source: String,

    /// Repository or project name
    pub repository: String,

    /// Branch name
    pub branch: String,

    /// Current status of the pipeline
    pub status: PipelineStatus,

    /// Build/run number
    pub build_number: u32,

    /// When the pipeline started
    pub started_at: DateTime<Utc>,

    /// When the pipeline finished (None if still running)
    pub finished_at: Option<DateTime<Utc>>,

    /// Duration of the pipeline execution
    pub duration: Option<Duration>,

    /// Individual stages/jobs in the pipeline
    pub stages: Vec<Stage>,

    /// URL to view the pipeline in the CI/CD platform
    pub url: String,

    /// Commit SHA that triggered this pipeline
    pub commit_sha: String,

    /// Commit message
    pub commit_message: String,

    /// Author of the commit
    pub author: String,
}

/// Status of a pipeline execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipelineStatus {
    /// Pipeline completed successfully
    Success,

    /// Pipeline is currently running
    Running,

    /// Pipeline failed
    Failed,

    /// Pipeline is queued/pending
    Pending,

    /// Pipeline was cancelled
    Cancelled,

    /// Pipeline was skipped
    Skipped,
}

impl PipelineStatus {
    /// Returns true if the pipeline is in a terminal state (not running/pending)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            PipelineStatus::Success
                | PipelineStatus::Failed
                | PipelineStatus::Cancelled
                | PipelineStatus::Skipped
        )
    }

    /// Returns true if the pipeline is active (running or pending)
    pub fn is_active(&self) -> bool {
        matches!(self, PipelineStatus::Running | PipelineStatus::Pending)
    }

    /// Returns true if the pipeline succeeded
    pub fn is_success(&self) -> bool {
        matches!(self, PipelineStatus::Success)
    }

    /// Returns true if the pipeline failed
    pub fn is_failure(&self) -> bool {
        matches!(self, PipelineStatus::Failed)
    }

    /// Returns a string representation of the status
    pub fn as_str(&self) -> &str {
        match self {
            PipelineStatus::Success => "Success",
            PipelineStatus::Running => "Running",
            PipelineStatus::Failed => "Failed",
            PipelineStatus::Pending => "Pending",
            PipelineStatus::Cancelled => "Cancelled",
            PipelineStatus::Skipped => "Skipped",
        }
    }

    /// Returns an emoji representation of the status
    pub fn emoji(&self) -> &str {
        match self {
            PipelineStatus::Success => "✓",
            PipelineStatus::Running => "●",
            PipelineStatus::Failed => "✗",
            PipelineStatus::Pending => "○",
            PipelineStatus::Cancelled => "⊘",
            PipelineStatus::Skipped => "⊙",
        }
    }
}

impl Pipeline {
    /// Calculate the duration of the pipeline
    pub fn calculate_duration(&self) -> Option<Duration> {
        if let Some(finished) = self.finished_at {
            Some(finished.signed_duration_since(self.started_at))
        } else if self.status == PipelineStatus::Running {
            Some(Utc::now().signed_duration_since(self.started_at))
        } else {
            None
        }
    }

    /// Get a short commit SHA (first 7 characters)
    pub fn short_commit_sha(&self) -> String {
        self.commit_sha.chars().take(7).collect()
    }

    /// Get a truncated commit message (first line, max 50 chars)
    pub fn short_commit_message(&self) -> String {
        let first_line = self.commit_message.lines().next().unwrap_or("");
        if first_line.len() > 50 {
            format!("{}...", &first_line[..47])
        } else {
            first_line.to_string()
        }
    }

    /// Count stages by status
    pub fn stage_counts(&self) -> StageStatusCounts {
        let mut counts = StageStatusCounts::default();
        for stage in &self.stages {
            match stage.status {
                crate::models::stage::StageStatus::Success => counts.success += 1,
                crate::models::stage::StageStatus::Running => counts.running += 1,
                crate::models::stage::StageStatus::Failed => counts.failed += 1,
                crate::models::stage::StageStatus::Pending => counts.pending += 1,
                crate::models::stage::StageStatus::Skipped => counts.skipped += 1,
            }
        }
        counts
    }
}

/// Stage status counts for a pipeline
#[derive(Debug, Default, Clone, Copy)]
pub struct StageStatusCounts {
    pub success: usize,
    pub running: usize,
    pub failed: usize,
    pub pending: usize,
    pub skipped: usize,
}

impl StageStatusCounts {
    pub fn total(&self) -> usize {
        self.success + self.running + self.failed + self.pending + self.skipped
    }
}
