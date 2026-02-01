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

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Execute an HTTP GET with automatic retry + exponential back-off.
    ///
    /// Transparently handles:
    ///   - 429 Too Many Requests  → respects `Retry-After` header
    ///   - 401 / 403              → surfaces an auth error immediately (no retry)
    ///   - 5xx                    → retries with back-off
    ///   - Network errors         → retries with back-off
    async fn get_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut attempt = 0u32;
        let mut delay = self.retry.base_delay;

        loop {
            let resp = self
                .http
                .get(url)
                .header("Authorization", format!("token {}", self.token))
                .header("Accept", "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28")
                .send();

            match resp.await {
                Ok(response) => {
                    let status = response.status();

                    // Auth failures are not retryable
                    if status == StatusCode::UNAUTHORIZED {
                        return Err(ApiError::AuthFailed(
                            "GitHub token is invalid or expired. \
                             Ensure your token has 'repo' and 'workflow' scopes."
                                .into(),
                        )
                        .into());
                    }
                    if status == StatusCode::FORBIDDEN {
                        return Err(ApiError::AuthFailed(
                            "GitHub API returned 403 Forbidden. \
                             Your token may lack the required scopes."
                                .into(),
                        )
                        .into());
                    }

                    // Rate-limited → respect Retry-After if present
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        let retry_after = response
                            .headers()
                            .get("Retry-After")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(60);

                        if attempt < self.retry.max_retries {
                            attempt += 1;
                            tokio::time::sleep(StdDuration::from_secs(retry_after)).await;
                            continue;
                        }
                        return Err(ApiError::RateLimitExceeded.into());
                    }

                    // Server errors are retryable
                    if status.is_server_error() {
                        if attempt < self.retry.max_retries {
                            attempt += 1;
                            tokio::time::sleep(delay.to_std().unwrap_or(StdDuration::from_secs(1)))
                                .await;
                            delay = std::cmp::min(delay * 2, self.retry.max_delay);
                            continue;
                        }
                        return Err(ApiError::ApiError {
                            status: status.as_u16(),
                            message: "GitHub API server error after retries".into(),
                        }
                        .into());
                    }

                    // Any other non-success status
                    if !status.is_success() {
                        return Err(ApiError::ApiError {
                            status: status.as_u16(),
                            message: format!("Unexpected status from GitHub API: {}", status),
                        }
                        .into());
                    }

                    return Ok(response);
                }
                Err(e) => {
                    if attempt < self.retry.max_retries {
                        attempt += 1;
                        tokio::time::sleep(delay.to_std().unwrap_or(StdDuration::from_secs(1)))
                            .await;
                        delay = std::cmp::min(delay * 2, self.retry.max_delay);
                        continue;
                    }
                    return Err(ApiError::Network(e.to_string()).into());
                }
            }
        }
    }

    /// Fetch the most recent workflow runs for a single repository.
    /// Returns up to `per_page` runs (GitHub caps at 100).
    async fn fetch_runs_for_repo(&self, repo: &str, per_page: u32) -> Result<Vec<WorkflowRun>> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs?per_page={}&status=",
            GITHUB_API_BASE, self.owner, repo, per_page
        );
        let resp = self.get_with_retry(&url).await?;
        let body: WorkflowRunsResponse = resp.json().await.map_err(|e| {
            ApiError::ParseFailed(format!("Failed to parse workflow runs: {}", e))
        })?;
        Ok(body.workflow_runs)
    }

    /// Fetch all jobs for a single workflow run.
    async fn fetch_jobs_for_run(&self, repo: &str, run_id: u64) -> Result<Vec<Job>> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}/jobs",
            GITHUB_API_BASE, self.owner, repo, run_id
        );
        let resp = self.get_with_retry(&url).await?;
        let body: JobsResponse = resp.json().await.map_err(|e| {
            ApiError::ParseFailed(format!("Failed to parse jobs: {}", e))
        })?;
        Ok(body.jobs)
    }

    /// Fetch the raw log text for a single job.
    async fn fetch_job_log(&self, job_id: u64) -> Result<String> {
        let url = format!("{}/repos/actions/jobs/{}/logs", GITHUB_API_BASE, job_id);
        // For logs the owner/repo aren't needed in the URL; GitHub routes by job_id.
        // However some installations require it. We use the simpler form here.
        let resp = self.get_with_retry(&url).await?;
        let text = resp.text().await.map_err(|e| {
            ApiError::ParseFailed(format!("Failed to read log body: {}", e))
        })?;
        Ok(text)
    }
}

// ---------------------------------------------------------------------------
// Mapping helpers: GitHub types → internal models
// ---------------------------------------------------------------------------

/// Map a GitHub workflow-run status + conclusion pair to our `PipelineStatus`.
fn map_pipeline_status(status: &str, conclusion: Option<&str>) -> PipelineStatus {
    match status {
        "completed" => match conclusion {
            Some("success") => PipelineStatus::Success,
            Some("failure") | Some("timed_out") => PipelineStatus::Failed,
            Some("cancelled") => PipelineStatus::Cancelled,
            Some("skipped") => PipelineStatus::Skipped,
            // action_required, stale, neutral, etc. – treat as pending
            _ => PipelineStatus::Pending,
        },
        "in_progress" => PipelineStatus::Running,
        "queued" | "waiting" | "requested" => PipelineStatus::Pending,
        _ => PipelineStatus::Pending,
    }
}

/// Map a GitHub job status + conclusion pair to our `StageStatus`.
fn map_stage_status(status: &str, conclusion: Option<&str>) -> StageStatus {
    match status {
        "completed" => match conclusion {
            Some("success") => StageStatus::Success,
            Some("failure") | Some("timed_out") => StageStatus::Failed,
            Some("cancelled") => StageStatus::Skipped,
            Some("skipped") => StageStatus::Skipped,
            _ => StageStatus::Pending,
        },
        "in_progress" => StageStatus::Running,
        "queued" | "waiting" => StageStatus::Pending,
        _ => StageStatus::Pending,
    }
}

/// Convert a `WorkflowRun` + its jobs into a `Pipeline`.
fn run_to_pipeline(run: &WorkflowRun, repo: &str, owner: &str, jobs: Vec<Job>) -> Pipeline {
    let status = map_pipeline_status(&run.status, run.conclusion.as_deref());

    let started_at = run.run_started_at.unwrap_or(run.created_at);
    let finished_at = if status.is_terminal() {
        Some(run.updated_at)
    } else {
        None
    };
    let duration = finished_at.map(|end| end.signed_duration_since(started_at));

    let stages: Vec<Stage> = jobs
        .iter()
        .enumerate()
        .map(|(i, job)| {
            let stage_status = map_stage_status(&job.status, job.conclusion.as_deref());
            let dur = match (job.started_at, job.completed_at) {
                (Some(s), Some(e)) => Some(e.signed_duration_since(s)),
                _ => None,
            };
            Stage {
                id: job.id.to_string(),
                name: job.name.clone(),
                status: stage_status,
                started_at: job.started_at,
                finished_at: job.completed_at,
                duration: dur,
                log_url: Some(format!(
                    "https://github.com/{}/{}/actions/runs/{}/jobs/{}",
                    owner, repo, run.id, job.id
                )),
                order: i,
            }
        })
        .collect();

    let commit_message = run
        .display_title
        .clone()
        .unwrap_or_else(|| "No commit message".into());
    let author = run
        .actor
        .as_ref()
        .map(|a| a.login.clone())
        .unwrap_or_else(|| "unknown".into());
    let workflow_name = run
        .name
        .clone()
        .unwrap_or_else(|| "Unnamed Workflow".into());

    Pipeline {
        id: format!("github-{}-{}", repo, run.id),
        name: workflow_name,
        source: "GitHub Actions".to_string(),
        repository: format!("{}/{}", owner, repo),
        branch: run.head_branch.clone().unwrap_or_else(|| "unknown".into()),
        status,
        build_number: run.run_number as u32,
        started_at,
        finished_at,
        duration,
        stages,
        url: run.html_url.clone(),
        commit_sha: run.head_sha.clone(),
        commit_message,
        author,
    }
}

/// Parse raw log text into structured `LogEntry` items.
/// GitHub CI logs contain timestamps like `2024-01-15T10:30:45.1234567Z` at
/// the start of each line.  We try to parse those; if parsing fails we still
/// keep the line as plain text.
fn parse_log_lines(raw: &str) -> Vec<LogEntry> {
    raw.lines()
        .enumerate()
        .map(|(i, line)| {
            // Attempt to split on the first space after an ISO-8601 timestamp
            let (timestamp, text) = if line.len() > 30 {
                if let Ok(ts) = DateTime::parse_from_rfc3339(&line[..30].trim()) {
                    (Some(ts.with_timezone(&Utc)), line[30..].trim_start().to_string())
                } else {
                    (None, line.to_string())
                }
            } else {
                (None, line.to_string())
            };

            LogEntry {
                line: i + 1,
                text,
                timestamp,
            }
        })
        .collect()
}