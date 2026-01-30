use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Represents a single stage/job in a pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    /// Unique identifier for this stage
    pub id: String,
    
    /// Name of the stage/job
    pub name: String,
    
    /// Current status of the stage
    pub status: StageStatus,
    
    /// When the stage started (None if not started yet)
    pub started_at: Option<DateTime<Utc>>,
    
    /// When the stage finished (None if still running or not started)
    pub finished_at: Option<DateTime<Utc>>,
    
    /// Duration of the stage execution
    pub duration: Option<Duration>,
    
    /// URL to view the stage logs
    pub log_url: Option<String>,
    
    /// Number of the stage in the pipeline sequence
    pub order: usize,
}

/// Status of a stage execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageStatus {
    /// Stage completed successfully
    Success,
    
    /// Stage is currently running
    Running,
    
    /// Stage failed
    Failed,
    
    /// Stage is queued/pending
    Pending,
    
    /// Stage was skipped
    Skipped,
}

impl StageStatus {
    /// Returns true if the stage is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StageStatus::Success | StageStatus::Failed | StageStatus::Skipped
        )
    }

    /// Returns true if the stage is active (running or pending)
    pub fn is_active(&self) -> bool {
        matches!(self, StageStatus::Running | StageStatus::Pending)
    }

    /// Returns true if the stage succeeded
    pub fn is_success(&self) -> bool {
        matches!(self, StageStatus::Success)
    }

    /// Returns true if the stage failed
    pub fn is_failure(&self) -> bool {
        matches!(self, StageStatus::Failed)
    }

    /// Returns a string representation of the status
    pub fn as_str(&self) -> &str {
        match self {
            StageStatus::Success => "Success",
            StageStatus::Running => "Running",
            StageStatus::Failed => "Failed",
            StageStatus::Pending => "Pending",
            StageStatus::Skipped => "Skipped",
        }
    }

    /// Returns an emoji representation of the status
    pub fn emoji(&self) -> &str {
        match self {
            StageStatus::Success => "✓",
            StageStatus::Running => "●",
            StageStatus::Failed => "✗",
            StageStatus::Pending => "○",
            StageStatus::Skipped => "⊙",
        }
    }
}

impl Stage {
    /// Calculate the duration of the stage
    pub fn calculate_duration(&self) -> Option<Duration> {
        match (self.started_at, self.finished_at) {
            (Some(start), Some(end)) => Some(end.signed_duration_since(start)),
            (Some(start), None) if self.status == StageStatus::Running => {
                Some(Utc::now().signed_duration_since(start))
            }
            _ => None,
        }
    }

    /// Create a new pending stage
    pub fn new_pending(id: String, name: String, order: usize) -> Self {
        Self {
            id,
            name,
            status: StageStatus::Pending,
            started_at: None,
            finished_at: None,
            duration: None,
            log_url: None,
            order,
        }
    }

    /// Create a new running stage
    pub fn new_running(id: String, name: String, order: usize, started_at: DateTime<Utc>) -> Self {
        Self {
            id,
            name,
            status: StageStatus::Running,
            started_at: Some(started_at),
            finished_at: None,
            duration: None,
            log_url: None,
            order,
        }
    }
}