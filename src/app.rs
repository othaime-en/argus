use crate::config::Config;

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
        // TODO: Add mock pipelines here
        Ok(())
    }
}