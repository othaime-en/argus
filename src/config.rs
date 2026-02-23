use config::{Config as ConfigBuilder, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::utils::error::{ConfigError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub sources: Vec<SourceConfig>,

    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: u64,

    #[serde(default)]
    pub ui: UiConfig,

    #[serde(default)]
    pub notification: NotificationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    pub name: String,

    #[serde(rename = "type")]
    pub source_type: String,

    /// Allow a source to be disabled without removing it from the config.
    #[serde(default = "default_true")]
    pub enabled: bool,

    pub url: Option<String>,

    pub token_env: Option<String>,

    /// Jenkins-specific: env var holding the username
    pub username_env: Option<String>,

    // ── GitHub ───────────────────────────────────────────────────────────────
    pub owner: Option<String>,
    pub repos: Option<Vec<String>>,

    // ── GitLab ───────────────────────────────────────────────────────────────
    pub group: Option<String>,
    pub projects: Option<Vec<String>>,

    // ── Jenkins ──────────────────────────────────────────────────────────────
    pub jobs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default = "default_theme")]
    pub theme: String,

    #[serde(default = "default_true")]
    pub show_timestamps: bool,

    #[serde(default = "default_max_log_lines")]
    pub max_log_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    #[serde(default)]
    pub desktop: bool,

    #[serde(default)]
    pub sound: bool,

    #[serde(default)]
    pub channels: Vec<String>,
}

fn default_refresh_interval() -> u64 { 30 }
fn default_theme() -> String { "default".to_string() }
fn default_true() -> bool { true }
fn default_max_log_lines() -> usize { 1000 }

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            show_timestamps: true,
            max_log_lines: default_max_log_lines(),
        }
    }
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self { desktop: false, sound: false, channels: Vec::new() }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut builder = ConfigBuilder::builder();

        let default_config = PathBuf::from("config/default.toml");
        if default_config.exists() {
            builder = builder.add_source(File::from(default_config));
        }

        if let Some(home) = dirs::home_dir() {
            let user_config = home.join(".config/argus/config.toml");
            if user_config.exists() {
                builder = builder.add_source(File::from(user_config));
            }
        }

        builder = builder.add_source(
            Environment::with_prefix("ARGUS").separator("_").try_parsing(true),
        );

        let config = builder
            .build()
            .map_err(|e| ConfigError::LoadFailed(e.to_string()))?;

        let parsed: Config = config
            .try_deserialize()
            .map_err(|e| ConfigError::ParseFailed(e.to_string()))?;

        parsed.validate()?;
        Ok(parsed)
    }

    fn validate(&self) -> Result<()> {
        if self.refresh_interval == 0 {
            return Err(ConfigError::InvalidConfig(
                "refresh_interval must be greater than 0".to_string(),
            )
            .into());
        }
        for source in &self.sources {
            if source.enabled {
                source.validate()?;
            }
        }
        Ok(())
    }

    pub fn default() -> Self {
        Self {
            sources: Vec::new(),
            refresh_interval: default_refresh_interval(),
            ui: UiConfig::default(),
            notification: NotificationConfig::default(),
        }
    }

    /// Return only sources that are enabled.
    pub fn active_sources(&self) -> impl Iterator<Item = &SourceConfig> {
        self.sources.iter().filter(|s| s.enabled)
    }
}

impl SourceConfig {
    fn validate(&self) -> Result<()> {
        match self.source_type.as_str() {
            "github" | "gitlab" | "jenkins" => {}
            other => {
                return Err(ConfigError::InvalidConfig(format!(
                    "Invalid source type '{}' in source '{}'. Must be 'github', 'gitlab', or 'jenkins'.",
                    other, self.name
                ))
                .into());
            }
        }

        if self.source_type == "github" {
            if self.owner.is_none() {
                return Err(ConfigError::MissingField(
                    format!("GitHub source '{}' requires 'owner'", self.name),
                ).into());
            }
            if self.repos.as_ref().map_or(true, |r| r.is_empty()) {
                return Err(ConfigError::MissingField(
                    format!("GitHub source '{}' requires at least one repo in 'repos'", self.name),
                ).into());
            }
        }

        if self.source_type == "gitlab" {
            if self.projects.as_ref().map_or(true, |p| p.is_empty()) {
                return Err(ConfigError::MissingField(
                    format!("GitLab source '{}' requires at least one entry in 'projects'", self.name),
                ).into());
            }
        }

        if self.source_type == "jenkins" {
            if self.url.is_none() {
                return Err(ConfigError::MissingField(
                    format!("Jenkins source '{}' requires 'url'", self.name),
                ).into());
            }
            if self.jobs.as_ref().map_or(true, |j| j.is_empty()) {
                return Err(ConfigError::MissingField(
                    format!("Jenkins source '{}' requires at least one job in 'jobs'", self.name),
                ).into());
            }
        }

        Ok(())
    }

    pub fn get_token(&self) -> Result<String> {
        if let Some(env_var) = &self.token_env {
            std::env::var(env_var)
                .map_err(|_| ConfigError::MissingEnvVar(env_var.clone()).into())
        } else {
            Err(ConfigError::MissingField(
                format!("source '{}' is missing 'token_env'", self.name),
            ).into())
        }
    }

    pub fn get_username(&self) -> Result<Option<String>> {
        if let Some(env_var) = &self.username_env {
            Ok(Some(
                std::env::var(env_var)
                    .map_err(|_| ConfigError::MissingEnvVar(env_var.clone()))?,
            ))
        } else {
            Ok(None)
        }
    }
}

use dirs;