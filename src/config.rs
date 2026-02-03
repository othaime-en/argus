use config::{Config as ConfigBuilder, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::utils::error::{ConfigError, Result};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Pipeline source configurations
    #[serde(default)]
    pub sources: Vec<SourceConfig>,
    
    /// Polling interval in seconds
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: u64,
    
    /// UI configuration
    #[serde(default)]
    pub ui: UiConfig,
    
    /// Notification configuration
    #[serde(default)]
    pub notification: NotificationConfig,
}

/// Configuration for a pipeline source (GitHub, GitLab, Jenkins, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    /// Name of this source (for display purposes)
    pub name: String,
    
    /// Type of source: "github", "gitlab", "jenkins"
    #[serde(rename = "type")]
    pub source_type: String,
    
    /// URL of the source (for self-hosted instances)
    pub url: Option<String>,
    
    /// Environment variable containing the auth token
    pub token_env: Option<String>,
    
    /// Environment variable containing the username (for Jenkins)
    pub username_env: Option<String>,
    
    /// GitHub-specific: organization/user owner
    pub owner: Option<String>,
    
    /// GitHub-specific: list of repositories to monitor
    pub repos: Option<Vec<String>>,
    
    /// GitLab-specific: group name
    pub group: Option<String>,
    
    /// GitLab-specific: list of projects to monitor
    pub projects: Option<Vec<String>>,
    
    /// Jenkins-specific: list of jobs to monitor
    pub jobs: Option<Vec<String>>,
}

/// UI-related configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Theme name: "default", "dark", "light", "monokai"
    #[serde(default = "default_theme")]
    pub theme: String,
    
    /// Show timestamps in pipeline list
    #[serde(default = "default_true")]
    pub show_timestamps: bool,
    
    /// Maximum number of log lines to display
    #[serde(default = "default_max_log_lines")]
    pub max_log_lines: usize,
}

/// Notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Enable desktop notifications
    #[serde(default)]
    pub desktop: bool,
    
    /// Enable sound alerts
    #[serde(default)]
    pub sound: bool,
    
    /// Notification channels
    #[serde(default)]
    pub channels: Vec<String>,
}

// Default value functions
fn default_refresh_interval() -> u64 {
    30
}

fn default_theme() -> String {
    "default".to_string()
}

fn default_true() -> bool {
    true
}

fn default_max_log_lines() -> usize {
    1000
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            show_timestamps: default_true(),
            max_log_lines: default_max_log_lines(),
        }
    }
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            desktop: false,
            sound: false,
            channels: Vec::new(),
        }
    }
}

impl Config {
    /// Load configuration from files and environment variables
    pub fn load() -> Result<Self> {
        let mut builder = ConfigBuilder::builder();

        // Start with default config file
        let default_config = PathBuf::from("config/default.toml");
        if default_config.exists() {
            builder = builder.add_source(File::from(default_config));
        }

        // Load user config file from ~/.config/argus/config.toml
        if let Some(home) = dirs::home_dir() {
            let user_config = home.join(".config/argus/config.toml");
            if user_config.exists() {
                builder = builder.add_source(File::from(user_config));
            }
        }

        // Override with environment variables (e.g., ARGUS_REFRESH_INTERVAL)
        builder = builder.add_source(
            Environment::with_prefix("ARGUS")
                .separator("_")
                .try_parsing(true),
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

    /// Validate the configuration
    fn validate(&self) -> Result<()> {
        // Validate refresh interval
        if self.refresh_interval == 0 {
            return Err(ConfigError::InvalidConfig(
                "refresh_interval must be greater than 0".to_string(),
            ).into());
        }

        // Validate sources
        for source in &self.sources {
            source.validate()?;
        }

        Ok(())
    }

    /// Create a default configuration (useful for testing)
    pub fn default() -> Self {
        Self {
            sources: Vec::new(),
            refresh_interval: default_refresh_interval(),
            ui: UiConfig::default(),
            notification: NotificationConfig::default(),
        }
    }
}

impl SourceConfig {
    /// Validate the source configuration
    fn validate(&self) -> Result<()> {
        // Validate source type
        match self.source_type.as_str() {
            "github" | "gitlab" | "jenkins" => {}
            _ => {
                return Err(ConfigError::InvalidConfig(format!(
                    "Invalid source type: {}. Must be 'github', 'gitlab', or 'jenkins'",
                    self.source_type
                )).into());
            }
        }

        // Validate GitHub-specific fields
        if self.source_type == "github" {
            if self.owner.is_none() {
                return Err(ConfigError::MissingField(
                    "GitHub source requires 'owner' field".to_string(),
                ).into());
            }
            if self.repos.is_none() || self.repos.as_ref().unwrap().is_empty() {
                return Err(ConfigError::MissingField(
                    "GitHub source requires 'repos' field with at least one repository".to_string(),
                ).into());
            }
        }

        // Validate GitLab-specific fields
        if self.source_type == "gitlab" {
            if self.group.is_none() {
                return Err(ConfigError::MissingField(
                    "GitLab source requires 'group' field".to_string(),
                ).into());
            }
            if self.projects.is_none() || self.projects.as_ref().unwrap().is_empty() {
                return Err(ConfigError::MissingField(
                    "GitLab source requires 'projects' field with at least one project".to_string(),
                ).into());
            }
        }

        // Validate Jenkins-specific fields
        if self.source_type == "jenkins" {
            if self.url.is_none() {
                return Err(ConfigError::MissingField(
                    "Jenkins source requires 'url' field".to_string(),
                ).into());
            }
            if self.jobs.is_none() || self.jobs.as_ref().unwrap().is_empty() {
                return Err(ConfigError::MissingField(
                    "Jenkins source requires 'jobs' field with at least one job".to_string(),
                ).into());
            }
        }

        Ok(())
    }

    /// Get the authentication token from environment variable
    pub fn get_token(&self) -> Result<String> {
        if let Some(env_var) = &self.token_env {
            std::env::var(env_var)
                .map_err(|_| ConfigError::MissingEnvVar(env_var.clone()).into())
        } else {
            Err(ConfigError::MissingField(
                "token_env field is required".to_string(),
            ).into())
        }
    }

    /// Get the username from environment variable (for Jenkins)
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

// Add dirs dependency for home directory
use dirs;