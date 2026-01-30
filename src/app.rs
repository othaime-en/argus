use crossterm::event::KeyCode;

use crate::config::Config;
use crate::models::Pipeline;
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
        // TODO: Add mock pipelines here
        Ok(())
    }

    /// Handle keyboard input
    pub fn handle_input(&mut self, key: KeyCode) {
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
}