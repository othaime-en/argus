use async_trait::async_trait;
use chrono::Duration;

use crate::models::Pipeline;
use crate::utils::Result;

/// A log entry from a CI/CD pipeline stage
#[derive(Debug, Clone)]
pub struct LogEntry {
    /// Line number in the log (1-indexed)
    pub line: usize,
    /// The raw log text for this line
    pub text: String,
    /// Optional timestamp embedded in the log line
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

/// Common interface that every CI/CD platform integration must satisfy.
///
/// Each method is async because real implementations perform network I/O.
/// The trait is object-safe via `async_trait` so we can store `Box<dyn CIPlatform>`
/// in the poller and state layers.
#[async_trait]
pub trait CIPlatform: Send + Sync {
    /// Fetch the most recent pipeline runs from this source.
    /// Implementations should return at most ~25 runs to keep the UI responsive.
    async fn fetch_pipelines(&self) -> Result<Vec<Pipeline>>;

    /// Fetch log output for a specific stage inside a pipeline.
    /// `pipeline_id` is the opaque ID stored on the `Pipeline` model.
    /// `stage_id` is the opaque ID stored on the `Stage` model.
    async fn fetch_logs(&self, pipeline_id: &str, stage_id: &str) -> Result<Vec<LogEntry>>;

    /// Perform a lightweight connectivity / auth check against the platform.
    /// Returns Ok(()) on success, or an error with a human-readable message.
    async fn test_connection(&self) -> Result<()>;

    /// Human-readable name shown in the UI (e.g. "GitHub Actions").
    fn platform_name(&self) -> &str;

    /// The user-defined source name from the config file.
    fn source_name(&self) -> &str;
}

/// Retry policy used by API clients.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (not counting the initial call)
    pub max_retries: u32,
    /// Base delay before the first retry; subsequent retries double this
    pub base_delay: Duration,
    /// Cap on how long any single delay can grow
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::seconds(1),
            max_delay: Duration::seconds(30),
        }
    }
}
