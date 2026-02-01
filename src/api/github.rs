use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::time::Duration as StdDuration;

use crate::api::traits::{CIPlatform, LogEntry, RetryPolicy};
use crate::config::SourceConfig;
use crate::models::{Pipeline, PipelineStatus, Stage, StageStatus};
use crate::utils::error::{ApiError, Result};

// ---------------------------------------------------------------------------
// GitHub API response shapes (only the fields we actually use)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct WorkflowRunsResponse {
    workflow_runs: Vec<WorkflowRun>,
}

#[derive(Deserialize, Debug, Clone)]
struct WorkflowRun {
    id: u64,
    name: Option<String>,
    head_branch: Option<String>,
    head_sha: String,
    status: String,               // "completed", "in_progress", "queued", "waiting"
    conclusion: Option<String>,   // "success", "failure", "cancelled", "skipped", "timed_out", null
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    run_started_at: Option<DateTime<Utc>>,
    html_url: String,
    display_title: Option<String>,
    // Nested actor for author
    actor: Option<Actor>,
    run_number: u64,
}

#[derive(Deserialize, Debug, Clone)]
struct Actor {
    login: String,
}

#[derive(Deserialize, Debug)]
struct JobsResponse {
    jobs: Vec<Job>,
}

#[derive(Deserialize, Debug, Clone)]
struct Job {
    id: u64,
    name: String,
    status: String,          // "queued", "in_progress", "completed"
    conclusion: Option<String>,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

const GITHUB_API_BASE: &str = "https://api.github.com";

/// GitHub Actions client.
///
/// One instance is created per `[[sources]]` entry of type `"github"` in the
/// user's config.  It owns the repos list and the bearer token resolved at
/// construction time.
pub struct GitHubClient {
    /// HTTP client shared across all requests
    http: Client,
    /// Personal access token (resolved from the env var at startup)
    token: String,
    /// GitHub org / user that owns the repositories
    owner: String,
    /// List of repo names (without the owner prefix)
    repos: Vec<String>,
    /// User-facing source name from config
    source: String,
    /// Retry policy for failed requests
    retry: RetryPolicy,
}

impl GitHubClient {
    /// Construct a new client from a validated `SourceConfig`.
    ///
    /// Reads the token from the environment variable specified in the config.
    /// Returns an error if the env var is missing or if required fields
    /// (owner, repos) are absent.
    pub fn new(source_config: &SourceConfig) -> Result<Self> {
        let token = source_config.get_token()?;

        let owner = source_config
            .owner
            .clone()
            .ok_or_else(|| ApiError::RequestFailed("GitHub source missing 'owner'".into()))?;

        let repos = source_config
            .repos
            .clone()
            .ok_or_else(|| ApiError::RequestFailed("GitHub source missing 'repos'".into()))?;

        // Pre-build an HTTP client with a 10 s timeout
        let http = Client::builder()
            .timeout(StdDuration::from_secs(10))
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        Ok(Self {
            http,
            token,
            owner,
            repos,
            source: source_config.name.clone(),
            retry: RetryPolicy::default(),
        })
    }
}