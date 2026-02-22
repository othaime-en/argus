use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::time::Duration as StdDuration;

use crate::api::traits::{CIPlatform, LogEntry, RetryPolicy};
use crate::config::SourceConfig;
use crate::models::{Pipeline, PipelineStatus, Stage, StageStatus};
use crate::utils::error::{ApiError, Result};

// ---------------------------------------------------------------------------
// GitLab API response shapes
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone)]
struct GitLabPipeline {
    id: u64,
    iid: u64, // project-scoped pipeline number
    #[serde(rename = "ref")]
    branch: Option<String>,
    status: String, // "created","waiting_for_resource","preparing","pending","running","success","failed","canceled","skipped","manual","scheduled"
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    web_url: String,
    sha: String,
    source: Option<String>, // "push","web","merge_request_event", etc.
}

#[derive(Deserialize, Debug, Clone)]
struct GitLabJob {
    id: u64,
    name: String,
    status: String,
    stage: String,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    web_url: String,
    commit: Option<GitLabCommit>,
    user: Option<GitLabUser>,
    duration: Option<f64>, // seconds
    allow_failure: bool,
}

#[derive(Deserialize, Debug, Clone)]
struct GitLabCommit {
    id: String,
    message: String,
    author_name: String,
}

#[derive(Deserialize, Debug, Clone)]
struct GitLabUser {
    username: String,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

pub struct GitLabClient {
    http: Client,
    token: String,
    base_url: String,
    projects: Vec<String>, // "group/project" slugs
    source: String,
    retry: RetryPolicy,
}

impl GitLabClient {
    pub fn new(source_config: &SourceConfig) -> Result<Self> {
        let token = source_config.get_token()?;

        let base_url = source_config
            .url
            .clone()
            .unwrap_or_else(|| "https://gitlab.com".to_string());
        let base_url = format!("{}/api/v4", base_url.trim_end_matches('/'));

        let projects = source_config
            .projects
            .clone()
            .ok_or_else(|| ApiError::RequestFailed("GitLab source missing 'projects'".into()))?;

        let http = Client::builder()
            .timeout(StdDuration::from_secs(10))
            .user_agent("Argus-Pipeline-Monitor/0.2.0")
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        Ok(Self {
            http,
            token,
            base_url,
            projects,
            source: source_config.name.clone(),
            retry: RetryPolicy::default(),
        })
    }

    async fn get_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut attempt = 0u32;
        let mut delay = self.retry.base_delay;

        loop {
            let resp = self
                .http
                .get(url)
                .header("PRIVATE-TOKEN", &self.token)
                .send()
                .await;

            match resp {
                Ok(response) => {
                    let status = response.status();

                    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                        return Err(ApiError::AuthFailed(
                            "GitLab token is invalid or lacks required scopes (api, read_api).".into(),
                        ).into());
                    }

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
                            message: "GitLab API server error after retries".into(),
                        }
                        .into());
                    }

                    if status == StatusCode::NOT_FOUND {
                        return Err(ApiError::NotFound(url.to_string()).into());
                    }

                    if !status.is_success() {
                        return Err(ApiError::ApiError {
                            status: status.as_u16(),
                            message: format!("Unexpected status from GitLab API: {}", status),
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

    /// URL-encode a project path ("group/project" → "group%2Fproject")
    fn encode_project(project: &str) -> String {
        project.replace('/', "%2F")
    }

    async fn fetch_pipelines_for_project(
        &self,
        project: &str,
    ) -> Result<Vec<GitLabPipeline>> {
        let encoded = Self::encode_project(project);
        let url = format!(
            "{}/projects/{}/pipelines?per_page=5&order_by=id&sort=desc",
            self.base_url, encoded
        );
        let resp = self.get_with_retry(&url).await?;
        let pipelines: Vec<GitLabPipeline> = resp
            .json()
            .await
            .map_err(|e| ApiError::ParseFailed(format!("Failed to parse GitLab pipelines: {}", e)))?;
        Ok(pipelines)
    }

    async fn fetch_jobs_for_pipeline(
        &self,
        project: &str,
        pipeline_id: u64,
    ) -> Result<Vec<GitLabJob>> {
        let encoded = Self::encode_project(project);
        let url = format!(
            "{}/projects/{}/pipelines/{}/jobs?per_page=100",
            self.base_url, encoded, pipeline_id
        );
        let resp = self.get_with_retry(&url).await?;
        let jobs: Vec<GitLabJob> = resp
            .json()
            .await
            .map_err(|e| ApiError::ParseFailed(format!("Failed to parse GitLab jobs: {}", e)))?;
        Ok(jobs)
    }

    async fn fetch_job_log_text(&self, project: &str, job_id: u64) -> Result<String> {
        let encoded = Self::encode_project(project);
        let url = format!(
            "{}/projects/{}/jobs/{}/trace",
            self.base_url, encoded, job_id
        );
        let resp = self.get_with_retry(&url).await?;
        let text = resp
            .text()
            .await
            .map_err(|e| ApiError::ParseFailed(format!("Failed to read log body: {}", e)))?;
        Ok(text)
    }
}

// ---------------------------------------------------------------------------
// Mapping helpers
// ---------------------------------------------------------------------------

fn map_pipeline_status(status: &str) -> PipelineStatus {
    match status {
        "success" => PipelineStatus::Success,
        "running" => PipelineStatus::Running,
        "failed" => PipelineStatus::Failed,
        "pending" | "created" | "waiting_for_resource" | "preparing" | "scheduled" => {
            PipelineStatus::Pending
        }
        "canceled" => PipelineStatus::Cancelled,
        "skipped" | "manual" => PipelineStatus::Skipped,
        _ => PipelineStatus::Pending,
    }
}

fn map_job_status(status: &str) -> StageStatus {
    match status {
        "success" => StageStatus::Success,
        "running" => StageStatus::Running,
        "failed" => StageStatus::Failed,
        "pending" | "created" | "waiting_for_resource" | "preparing" | "scheduled" => {
            StageStatus::Pending
        }
        "canceled" | "skipped" | "manual" => StageStatus::Skipped,
        _ => StageStatus::Pending,
    }
}

fn gl_pipeline_to_pipeline(
    gl: &GitLabPipeline,
    jobs: Vec<GitLabJob>,
    project: &str,
) -> Pipeline {
    let status = map_pipeline_status(&gl.status);
    let started_at = gl.started_at.unwrap_or(gl.created_at);
    let duration = match (gl.started_at, gl.finished_at) {
        (Some(s), Some(e)) => Some(e.signed_duration_since(s)),
        _ => None,
    };

    // Grab commit info from the first job that has it
    let (commit_message, author) = jobs
        .iter()
        .find_map(|j| {
            j.commit.as_ref().map(|c| (c.message.clone(), c.author_name.clone()))
        })
        .unwrap_or_else(|| ("No commit message".into(), "unknown".into()));

    // Build stages in stage order then job order
    let mut stages: Vec<Stage> = jobs
        .iter()
        .enumerate()
        .map(|(i, job)| {
            let dur = match (job.started_at, job.finished_at) {
                (Some(s), Some(e)) => Some(e.signed_duration_since(s)),
                _ => job.duration.map(|d| chrono::Duration::milliseconds((d * 1000.0) as i64)),
            };
            Stage {
                id: job.id.to_string(),
                name: format!("{} / {}", job.stage, job.name),
                status: map_job_status(&job.status),
                started_at: job.started_at,
                finished_at: job.finished_at,
                duration: dur,
                log_url: Some(job.web_url.clone()),
                order: i,
            }
        })
        .collect();
    stages.sort_by_key(|s| s.order);

    Pipeline {
        id: format!("gitlab-{}-{}", project.replace('/', "-"), gl.id),
        name: format!("Pipeline #{}", gl.iid),
        source: "GitLab CI".to_string(),
        repository: project.to_string(),
        branch: gl.branch.clone().unwrap_or_else(|| "unknown".into()),
        status,
        build_number: gl.iid as u32,
        started_at,
        finished_at: gl.finished_at,
        duration,
        stages,
        url: gl.web_url.clone(),
        commit_sha: gl.sha.clone(),
        commit_message,
        author,
    }
}

fn parse_log_lines(raw: &str) -> Vec<LogEntry> {
    raw.lines()
        .enumerate()
        .map(|(i, line)| LogEntry {
            line: i + 1,
            text: line.to_string(),
            timestamp: None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// CIPlatform implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl CIPlatform for GitLabClient {
    async fn fetch_pipelines(&self) -> Result<Vec<Pipeline>> {
        let mut all = Vec::new();

        for project in &self.projects {
            let pipelines = match self.fetch_pipelines_for_project(project).await {
                Ok(p) => p,
                Err(_) => continue,
            };

            for gl_pipeline in &pipelines {
                let jobs = match self
                    .fetch_jobs_for_pipeline(project, gl_pipeline.id)
                    .await
                {
                    Ok(j) => j,
                    Err(_) => vec![],
                };
                all.push(gl_pipeline_to_pipeline(gl_pipeline, jobs, project));
            }
        }

        all.sort_by(|a, b| {
            let order = |s: &PipelineStatus| match s {
                PipelineStatus::Running => 0,
                PipelineStatus::Pending => 1,
                PipelineStatus::Failed => 2,
                PipelineStatus::Success => 3,
                PipelineStatus::Cancelled => 4,
                PipelineStatus::Skipped => 5,
            };
            order(&a.status)
                .cmp(&order(&b.status))
                .then_with(|| b.started_at.cmp(&a.started_at))
        });

        Ok(all)
    }

    async fn fetch_logs(&self, pipeline_id: &str, stage_id: &str) -> Result<Vec<LogEntry>> {
        // pipeline_id format: "gitlab-{project_dashes}-{run_id}"
        // We need the project slug, which we can find by checking our projects list.
        // The stage_id is the numeric GitLab job ID.
        let job_id: u64 = stage_id
            .parse()
            .map_err(|_| ApiError::RequestFailed(format!("Invalid stage_id: {}", stage_id)))?;

        // Find which project this pipeline belongs to by matching the id prefix
        let project = self
            .projects
            .iter()
            .find(|p| {
                let prefix = format!("gitlab-{}-", p.replace('/', "-"));
                pipeline_id.starts_with(&prefix)
            })
            .ok_or_else(|| {
                ApiError::RequestFailed(format!(
                    "Cannot determine project for pipeline_id: {}",
                    pipeline_id
                ))
            })?;

        let raw = self.fetch_job_log_text(project, job_id).await?;
        Ok(parse_log_lines(&raw))
    }

    async fn test_connection(&self) -> Result<()> {
        let url = format!("{}/user", self.base_url);
        self.get_with_retry(&url).await?;
        Ok(())
    }

    fn platform_name(&self) -> &str {
        "GitLab CI"
    }

    fn source_name(&self) -> &str {
        &self.source
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_pipeline_status() {
        assert_eq!(map_pipeline_status("success"), PipelineStatus::Success);
        assert_eq!(map_pipeline_status("running"), PipelineStatus::Running);
        assert_eq!(map_pipeline_status("failed"), PipelineStatus::Failed);
        assert_eq!(map_pipeline_status("pending"), PipelineStatus::Pending);
        assert_eq!(map_pipeline_status("canceled"), PipelineStatus::Cancelled);
        assert_eq!(map_pipeline_status("skipped"), PipelineStatus::Skipped);
        assert_eq!(map_pipeline_status("manual"), PipelineStatus::Skipped);
    }

    #[test]
    fn test_map_job_status() {
        assert_eq!(map_job_status("success"), StageStatus::Success);
        assert_eq!(map_job_status("running"), StageStatus::Running);
        assert_eq!(map_job_status("failed"), StageStatus::Failed);
        assert_eq!(map_job_status("canceled"), StageStatus::Skipped);
    }

    #[test]
    fn test_encode_project() {
        assert_eq!(GitLabClient::encode_project("group/project"), "group%2Fproject");
        assert_eq!(GitLabClient::encode_project("a/b/c"), "a%2Fb%2Fc");
    }

    #[test]
    fn test_parse_log_lines() {
        let raw = "line one\nline two\nline three";
        let entries = parse_log_lines(raw);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].line, 1);
        assert_eq!(entries[0].text, "line one");
        assert_eq!(entries[2].line, 3);
    }

    #[test]
    fn test_gl_pipeline_to_pipeline_basic() {
        let gl = GitLabPipeline {
            id: 999,
            iid: 42,
            branch: Some("develop".into()),
            status: "success".into(),
            created_at: Utc::now(),
            updated_at: None,
            started_at: None,
            finished_at: None,
            web_url: "https://gitlab.com/org/proj/-/pipelines/999".into(),
            sha: "deadbeef1234".into(),
            source: Some("push".into()),
        };
        let pipeline = gl_pipeline_to_pipeline(&gl, vec![], "org/proj");
        assert_eq!(pipeline.id, "gitlab-org-proj-999");
        assert_eq!(pipeline.build_number, 42);
        assert_eq!(pipeline.branch, "develop");
        assert_eq!(pipeline.status, PipelineStatus::Success);
        assert_eq!(pipeline.repository, "org/proj");
    }
}