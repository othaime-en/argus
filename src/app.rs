use chrono::{Duration, Utc};

use crate::config::Config;
use crate::models::{Pipeline, PipelineStatus, Stage, StageStatus};
use crate::utils::Result;

/// Main application state
#[derive(Debug)]
pub struct App {
    /// Application configuration
    pub config: Config,
    
    /// List of all pipelines
    pub pipelines: Vec<Pipeline>,
    
    /// Index of the currently selected pipeline
    pub selected_pipeline: usize,
    
    /// Current focus area
    pub focus: Focus,
    
    /// Refresh interval in seconds
    pub refresh_interval: u64,
    
    /// Whether the application should quit
    pub should_quit: bool,
}

/// Focus areas in the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    PipelineList,
    Details,
}

impl App {
    /// Create a new application instance
    pub fn new(config: Config) -> Self {
        let refresh_interval = config.refresh_interval;
        
        Self {
            config,
            pipelines: Vec::new(),
            selected_pipeline: 0,
            focus: Focus::PipelineList,
            refresh_interval,
            should_quit: false,
        }
    }

    /// Initialize the application with mock data for Phase 1
    pub fn init_with_mock_data(&mut self) -> Result<()> {
        // Create some mock pipelines for testing the UI
        self.pipelines = vec![
            create_mock_pipeline(
                "backend-api",
                "main",
                PipelineStatus::Success,
                1234,
                "Fix authentication bug",
                "Alice Smith",
            ),
            create_mock_pipeline(
                "frontend-app",
                "develop",
                PipelineStatus::Running,
                5678,
                "Update dashboard UI",
                "Bob Jones",
            ),
            create_mock_pipeline(
                "mobile-app",
                "feature/login",
                PipelineStatus::Failed,
                9012,
                "Add biometric authentication",
                "Carol White",
            ),
            create_mock_pipeline(
                "data-pipeline",
                "main",
                PipelineStatus::Pending,
                3456,
                "Optimize ETL process",
                "David Brown",
            ),
            create_mock_pipeline(
                "infrastructure",
                "main",
                PipelineStatus::Success,
                7890,
                "Update Kubernetes configs",
                "Eve Davis",
            ),
        ];

        Ok(())
    }

    /// Handle keyboard input
    pub fn handle_input(&mut self, key: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;

        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous_pipeline();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next_pipeline();
            }
            KeyCode::Char('r') => {
                // Refresh will be implemented in Phase 2
                // For now, just acknowledge the key press
            }
            KeyCode::Enter => {
                // Toggle focus between pipeline list and details
                self.focus = match self.focus {
                    Focus::PipelineList => Focus::Details,
                    Focus::Details => Focus::PipelineList,
                };
            }
            _ => {}
        }
    }

    /// Select the previous pipeline in the list
    fn previous_pipeline(&mut self) {
        if self.pipelines.is_empty() {
            return;
        }
        
        if self.selected_pipeline > 0 {
            self.selected_pipeline -= 1;
        } else {
            self.selected_pipeline = self.pipelines.len() - 1;
        }
    }

    /// Select the next pipeline in the list
    fn next_pipeline(&mut self) {
        if self.pipelines.is_empty() {
            return;
        }
        
        if self.selected_pipeline < self.pipelines.len() - 1 {
            self.selected_pipeline += 1;
        } else {
            self.selected_pipeline = 0;
        }
    }

    /// Get the currently selected pipeline
    pub fn selected(&self) -> Option<&Pipeline> {
        self.pipelines.get(self.selected_pipeline)
    }

    /// Refresh pipeline data (placeholder for Phase 2)
    pub async fn refresh(&mut self) -> Result<()> {
        // This will be implemented in Phase 2 with actual API calls
        Ok(())
    }
}

/// Create a mock pipeline for testing
fn create_mock_pipeline(
    name: &str,
    branch: &str,
    status: PipelineStatus,
    build_number: u32,
    commit_message: &str,
    author: &str,
) -> Pipeline {
    let now = Utc::now();
    let started_at = now - Duration::minutes(15);
    
    let (finished_at, duration) = if status.is_terminal() {
        let finished = started_at + Duration::minutes(10);
        (Some(finished), Some(finished - started_at))
    } else {
        (None, None)
    };

    let stages = create_mock_stages(status);

    Pipeline {
        id: format!("mock-{}-{}", name, build_number),
        name: name.to_string(),
        source: "GitHub Actions".to_string(),
        repository: format!("myorg/{}", name),
        branch: branch.to_string(),
        status,
        build_number,
        started_at,
        finished_at,
        duration,
        stages,
        url: format!("https://github.com/myorg/{}/actions/runs/{}", name, build_number),
        commit_sha: format!("abc123{}", build_number),
        commit_message: commit_message.to_string(),
        author: author.to_string(),
    }
}

/// Create mock stages based on pipeline status
fn create_mock_stages(pipeline_status: PipelineStatus) -> Vec<Stage> {
    let stage_names = vec!["Build", "Test", "Lint", "Deploy"];
    
    stage_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let status = match pipeline_status {
                PipelineStatus::Success => StageStatus::Success,
                PipelineStatus::Running => {
                    if i < 2 {
                        StageStatus::Success
                    } else if i == 2 {
                        StageStatus::Running
                    } else {
                        StageStatus::Pending
                    }
                }
                PipelineStatus::Failed => {
                    if i < 2 {
                        StageStatus::Success
                    } else if i == 2 {
                        StageStatus::Failed
                    } else {
                        StageStatus::Skipped
                    }
                }
                PipelineStatus::Pending => StageStatus::Pending,
                PipelineStatus::Cancelled => StageStatus::Skipped,
                PipelineStatus::Skipped => StageStatus::Skipped,
            };

            let now = Utc::now();
            let started_at = if status != StageStatus::Pending {
                Some(now - Duration::minutes(15 - i as i64 * 3))
            } else {
                None
            };

            let finished_at = if status.is_terminal() && status != StageStatus::Skipped {
                Some(now - Duration::minutes(12 - i as i64 * 3))
            } else {
                None
            };

            let duration = match (started_at, finished_at) {
                (Some(start), Some(end)) => Some(end - start),
                _ => None,
            };

            Stage {
                id: format!("stage-{}", i),
                name: name.to_string(),
                status,
                started_at,
                finished_at,
                duration,
                log_url: Some(format!("https://example.com/logs/{}", i)),
                order: i,
            }
        })
        .collect()
}