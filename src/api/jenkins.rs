use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::time::Duration as StdDuration;

use crate::api::traits::{CIPlatform, LogEntry, RetryPolicy};
use crate::config::SourceConfig;
use crate::models::{Pipeline, PipelineStatus, Stage, StageStatus};
use crate::utils::error::{ApiError, Result};

// ---------------------------------------------------------------------------
// Jenkins API response shapes
// ---------------------------------------------------------------------------

/// Top-level jobs list response
#[derive(Deserialize, Debug)]
struct JobsResponse {
    jobs: Vec<JobSummary>,
}

#[derive(Deserialize, Debug, Clone)]
struct JobSummary {
    name: String,
    url: String,
    #[serde(rename = "lastBuild")]
    last_build: Option<BuildRef>,
}

#[derive(Deserialize, Debug, Clone)]
struct BuildRef {
    number: u32,
    url: String,
}

/// Full build details
#[derive(Deserialize, Debug)]
struct BuildDetail {
    number: u32,
    result: Option<String>, // "SUCCESS","FAILURE","ABORTED","UNSTABLE", null if in progress
    #[serde(rename = "inProgress")]
    in_progress: bool,
    timestamp: u64, // Unix millis
    duration: u64,  // millis (0 if still running)
    #[serde(rename = "estimatedDuration")]
    estimated_duration: u64,
    url: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "fullDisplayName")]
    full_display_name: Option<String>,
    actions: Vec<BuildAction>,
}

#[derive(Deserialize, Debug)]
struct BuildAction {
    #[serde(rename = "_class")]
    class: Option<String>,
    causes: Option<Vec<Cause>>,
    #[serde(rename = "remoteUrls")]
    remote_urls: Option<Vec<String>>,
    #[serde(rename = "lastBuiltRevision")]
    last_built_revision: Option<Revision>,
    parameters: Option<Vec<Parameter>>,
}

#[derive(Deserialize, Debug)]
struct Cause {
    #[serde(rename = "shortDescription")]
    short_description: Option<String>,
    #[serde(rename = "userId")]
    user_id: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Revision {
    #[serde(rename = "SHA1")]
    sha1: Option<String>,
    branch: Option<Vec<Branch>>,
}

#[derive(Deserialize, Debug)]
struct Branch {
    name: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Parameter {
    name: Option<String>,
    value: Option<serde_json::Value>,
}

/// Workflow API (Blue Ocean / Pipeline stage API)
#[derive(Deserialize, Debug)]
struct WfapiDescribe {
    stages: Vec<WfapiStage>,
}

#[derive(Deserialize, Debug)]
struct WfapiStage {
    id: String,
    name: String,
    status: String, // "SUCCESS","FAILED","IN_PROGRESS","NOT_EXECUTED","PAUSED"
    #[serde(rename = "startTimeMillis")]
    start_time_millis: Option<u64>,
    #[serde(rename = "durationMillis")]
    duration_millis: Option<u64>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

pub struct JenkinsClient {
    http: Client,
    base_url: String,
    credentials: Option<(String, String)>, // (username, api_token)
    jobs: Vec<String>,
    source: String,
    retry: RetryPolicy,
}

impl JenkinsClient {
    pub fn new(source_config: &SourceConfig) -> Result<Self> {
        let base_url = source_config
            .url
            .clone()
            .ok_or_else(|| ApiError::RequestFailed("Jenkins source missing 'url'".into()))?;
        let base_url = base_url.trim_end_matches('/').to_string();

        let jobs = source_config
            .jobs
            .clone()
            .ok_or_else(|| ApiError::RequestFailed("Jenkins source missing 'jobs'".into()))?;

        // Username + token are optional (some Jenkins allow anonymous read)
        let credentials = match (
            source_config.get_username()?,
            source_config.get_token().ok(),
        ) {
            (Some(user), Some(token)) => Some((user, token)),
            _ => None,
        };

        let http = Client::builder()
            .timeout(StdDuration::from_secs(15))
            .user_agent("Argus-Pipeline-Monitor/0.2.0")
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;

        Ok(Self {
            http,
            base_url,
            credentials,
            jobs,
            source: source_config.name.clone(),
            retry: RetryPolicy::default(),
        })
    }

    fn apply_auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some((user, token)) = &self.credentials {
            req.basic_auth(user, Some(token))
        } else {
            req
        }
    }

    async fn get_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut attempt = 0u32;
        let mut delay = self.retry.base_delay;

        loop {
            let req = self.apply_auth(self.http.get(url));
            match req.send().await {
                Ok(response) => {
                    let status = response.status();

                    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                        return Err(ApiError::AuthFailed(
                            "Jenkins authentication failed. Check username and API token."
                                .into(),
                        )
                        .into());
                    }

                    if status == StatusCode::NOT_FOUND {
                        return Err(ApiError::NotFound(url.to_string()).into());
                    }

                    if status.is_server_error() && attempt < self.retry.max_retries {
                        attempt += 1;
                        tokio::time::sleep(delay.to_std().unwrap_or(StdDuration::from_secs(1)))
                            .await;
                        delay = std::cmp::min(delay * 2, self.retry.max_delay);
                        continue;
                    }

                    if !status.is_success() {
                        return Err(ApiError::ApiError {
                            status: status.as_u16(),
                            message: format!("Unexpected status from Jenkins: {}", status),
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

    async fn fetch_build_detail(&self, job_name: &str, build_num: u32) -> Result<BuildDetail> {
        let url = format!(
            "{}/job/{}/{}/api/json?tree=number,result,inProgress,timestamp,duration,estimatedDuration,url,displayName,fullDisplayName,actions[causes[shortDescription,userId],remoteUrls,lastBuiltRevision[SHA1,branch[name]],parameters[name,value]]",
            self.base_url, job_name, build_num
        );
        let resp = self.get_with_retry(&url).await?;
        resp.json::<BuildDetail>()
            .await
            .map_err(|e| ApiError::ParseFailed(format!("Failed to parse build detail: {}", e)).into())
    }

    async fn fetch_pipeline_stages(&self, job_name: &str, build_num: u32) -> Result<Vec<WfapiStage>> {
        let url = format!(
            "{}/job/{}/{}/wfapi/describe",
            self.base_url, job_name, build_num
        );
        match self.get_with_retry(&url).await {
            Ok(resp) => {
                let wf: WfapiDescribe = resp
                    .json()
                    .await
                    .map_err(|e| ApiError::ParseFailed(format!("Failed to parse wfapi: {}", e)))?;
                Ok(wf.stages)
            }
            // Not all Jenkins jobs expose wfapi; fall back to empty stages gracefully
            Err(_) => Ok(vec![]),
        }
    }

    async fn fetch_console_text(&self, job_name: &str, build_num: u32) -> Result<String> {
        let url = format!(
            "{}/job/{}/{}/consoleText",
            self.base_url, job_name, build_num
        );
        let resp = self.get_with_retry(&url).await?;
        resp.text()
            .await
            .map_err(|e| ApiError::ParseFailed(format!("Failed to read console output: {}", e)).into())
    }
}

// ---------------------------------------------------------------------------
// Mapping helpers
// ---------------------------------------------------------------------------

fn map_build_status(result: Option<&str>, in_progress: bool) -> PipelineStatus {
    if in_progress {
        return PipelineStatus::Running;
    }
    match result {
        Some("SUCCESS") => PipelineStatus::Success,
        Some("FAILURE") | Some("UNSTABLE") => PipelineStatus::Failed,
        Some("ABORTED") => PipelineStatus::Cancelled,
        Some("NOT_BUILT") => PipelineStatus::Skipped,
        None => PipelineStatus::Pending,
        _ => PipelineStatus::Pending,
    }
}

fn map_stage_status(status: &str) -> StageStatus {
    match status {
        "SUCCESS" => StageStatus::Success,
        "FAILED" | "UNSTABLE" => StageStatus::Failed,
        "IN_PROGRESS" => StageStatus::Running,
        "PAUSED" => StageStatus::Pending,
        "NOT_EXECUTED" => StageStatus::Skipped,
        _ => StageStatus::Pending,
    }
}

fn millis_to_datetime(millis: u64) -> DateTime<Utc> {
    let secs = (millis / 1000) as i64;
    let nanos = ((millis % 1000) * 1_000_000) as u32;
    Utc.timestamp_opt(secs, nanos).single().unwrap_or_else(Utc::now)
}

fn extract_branch(detail: &BuildDetail) -> String {
    for action in &detail.actions {
        if let Some(rev) = &action.last_built_revision {
            if let Some(branches) = &rev.branch {
                if let Some(branch) = branches.first() {
                    if let Some(name) = &branch.name {
                        // Strip "refs/remotes/origin/" prefix if present
                        let clean = name
                            .strip_prefix("refs/remotes/origin/")
                            .or_else(|| name.strip_prefix("refs/heads/"))
                            .unwrap_or(name);
                        return clean.to_string();
                    }
                }
            }
        }
    }
    "unknown".to_string()
}

fn extract_commit_sha(detail: &BuildDetail) -> String {
    for action in &detail.actions {
        if let Some(rev) = &action.last_built_revision {
            if let Some(sha) = &rev.sha1 {
                return sha.clone();
            }
        }
    }
    String::new()
}

fn extract_author(detail: &BuildDetail) -> String {
    for action in &detail.actions {
        if let Some(causes) = &action.causes {
            for cause in causes {
                if let Some(user) = &cause.user_id {
                    return user.clone();
                }
            }
        }
    }
    "Jenkins".to_string()
}

fn build_to_pipeline(
    detail: BuildDetail,
    stages: Vec<WfapiStage>,
    job_name: &str,
    base_url: &str,
) -> Pipeline {
    let status = map_build_status(detail.result.as_deref(), detail.in_progress);
    let started_at = millis_to_datetime(detail.timestamp);
    let duration = if detail.duration > 0 {
        Some(Duration::milliseconds(detail.duration as i64))
    } else {
        None
    };
    let finished_at = if let Some(dur) = duration {
        Some(started_at + dur)
    } else {
        None
    };

    let pipeline_stages: Vec<Stage> = stages
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let stage_start = s.start_time_millis.map(millis_to_datetime);
            let stage_dur = s.duration_millis.map(|d| Duration::milliseconds(d as i64));
            let stage_end = match (stage_start, stage_dur) {
                (Some(st), Some(d)) => Some(st + d),
                _ => None,
            };
            Stage {
                // Encode job name + stage id so fetch_logs can reconstruct the URL
                id: format!("{}__{}_{}", job_name, detail.number, s.id),
                name: s.name.clone(),
                status: map_stage_status(&s.status),
                started_at: stage_start,
                finished_at: stage_end,
                duration: stage_dur,
                log_url: Some(format!(
                    "{}/job/{}/{}/execution/node/{}/log",
                    base_url, job_name, detail.number, s.id
                )),
                order: i,
            }
        })
        .collect();

    let branch = extract_branch(&detail);
    let commit_sha = extract_commit_sha(&detail);
    let author = extract_author(&detail);
    let display_name = detail
        .full_display_name
        .or(detail.display_name)
        .unwrap_or_else(|| format!("{} #{}", job_name, detail.number));

    Pipeline {
        id: format!("jenkins-{}-{}", job_name, detail.number),
        name: job_name.to_string(),
        source: "Jenkins".to_string(),
        repository: job_name.to_string(),
        branch,
        status,
        build_number: detail.number,
        started_at,
        finished_at,
        duration,
        stages: pipeline_stages,
        url: detail.url,
        commit_sha,
        commit_message: display_name,
        author,
    }
}

// ---------------------------------------------------------------------------
// CIPlatform implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl CIPlatform for JenkinsClient {
    async fn fetch_pipelines(&self) -> Result<Vec<Pipeline>> {
        let mut all = Vec::new();

        for job_name in &self.jobs {
            // Fetch the last build number
            let summary_url = format!(
                "{}/job/{}/api/json?tree=name,url,lastBuild[number,url]",
                self.base_url, job_name
            );
            let summary_resp = match self.get_with_retry(&summary_url).await {
                Ok(r) => r,
                Err(_) => continue,
            };
            let summary: JobSummary = match summary_resp.json().await {
                Ok(s) => s,
                Err(_) => continue,
            };

            let last_build = match summary.last_build {
                Some(b) => b,
                None => continue, // job has never run
            };

            let detail = match self.fetch_build_detail(job_name, last_build.number).await {
                Ok(d) => d,
                Err(_) => continue,
            };

            let stages = self
                .fetch_pipeline_stages(job_name, last_build.number)
                .await
                .unwrap_or_default();

            all.push(build_to_pipeline(detail, stages, job_name, &self.base_url));
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

    /// Fetch console output for a Jenkins build.
    /// `stage_id` format: "{job_name}__{build_num}_{wfapi_stage_id}"
    /// We use this to reconstruct the job name + build number needed for the URL.
    async fn fetch_logs(&self, pipeline_id: &str, stage_id: &str) -> Result<Vec<LogEntry>> {
        // pipeline_id format: "jenkins-{job_name}-{build_num}"
        // Extract job name from pipeline_id
        let stripped = pipeline_id
            .strip_prefix("jenkins-")
            .ok_or_else(|| ApiError::RequestFailed(format!("Invalid pipeline_id: {}", pipeline_id)))?;

        // Find the last '-' to split job_name from build number
        let last_dash = stripped
            .rfind('-')
            .ok_or_else(|| ApiError::RequestFailed(format!("Cannot parse pipeline_id: {}", pipeline_id)))?;
        let job_name = &stripped[..last_dash];
        let build_num: u32 = stripped[last_dash + 1..]
            .parse()
            .map_err(|_| ApiError::RequestFailed(format!("Invalid build number in: {}", pipeline_id)))?;

        let raw = self.fetch_console_text(job_name, build_num).await?;

        Ok(raw
            .lines()
            .enumerate()
            .map(|(i, line)| LogEntry {
                line: i + 1,
                text: line.to_string(),
                timestamp: None,
            })
            .collect())
    }

    async fn test_connection(&self) -> Result<()> {
        let url = format!("{}/api/json?tree=mode", self.base_url);
        self.get_with_retry(&url).await?;
        Ok(())
    }

    fn platform_name(&self) -> &str {
        "Jenkins"
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
    fn test_map_build_status() {
        assert_eq!(map_build_status(Some("SUCCESS"), false), PipelineStatus::Success);
        assert_eq!(map_build_status(Some("FAILURE"), false), PipelineStatus::Failed);
        assert_eq!(map_build_status(Some("UNSTABLE"), false), PipelineStatus::Failed);
        assert_eq!(map_build_status(Some("ABORTED"), false), PipelineStatus::Cancelled);
        assert_eq!(map_build_status(None, true), PipelineStatus::Running);
        assert_eq!(map_build_status(None, false), PipelineStatus::Pending);
    }

    #[test]
    fn test_map_stage_status() {
        assert_eq!(map_stage_status("SUCCESS"), StageStatus::Success);
        assert_eq!(map_stage_status("FAILED"), StageStatus::Failed);
        assert_eq!(map_stage_status("IN_PROGRESS"), StageStatus::Running);
        assert_eq!(map_stage_status("NOT_EXECUTED"), StageStatus::Skipped);
        assert_eq!(map_stage_status("PAUSED"), StageStatus::Pending);
    }

    #[test]
    fn test_millis_to_datetime() {
        let dt = millis_to_datetime(1_700_000_000_000);
        assert!(dt.timestamp() > 0);
    }
}