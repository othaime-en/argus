use crate::api::LogEntry;
use crate::config::Config;
use crate::services::PollUpdate;
use crate::state::{AppState, SourceStatus};

/// Main application state – lives on the main thread and is mutated only
/// by the event loop.  All async work happens in background tasks; results
/// arrive via channels.
pub struct App {
    /// Static configuration loaded at startup
    pub config: Config,

    /// Live pipeline + error state, updated by poll messages
    pub state: AppState,

    /// Index of the currently highlighted pipeline in the sorted list
    pub selected_pipeline: usize,

    /// Which UI panel currently has keyboard focus
    pub focus: Focus,

    /// Logs for the currently selected pipeline/stage.
    /// `None` means "not yet fetched"; `Some(vec![])` means "fetched, empty".
    pub logs: Option<Vec<LogEntry>>,

    /// Which stage index inside the selected pipeline we are viewing logs for.
    /// Used to know whether to re-fetch when the user presses 'l'.
    pub log_stage_index: Option<usize>,

    /// Vertical scroll offset inside the log viewer
    pub log_scroll: usize,

    /// Whether the error panel is visible
    pub show_errors: bool,

    /// Refresh interval in seconds (copied from config for quick access)
    pub refresh_interval: u64,

    /// Set to true when the user presses 'q'
    pub should_quit: bool,

    /// Set to true when the user presses 'r' – the event loop will trigger
    /// an immediate re-poll and then clear this flag.
    pub force_refresh: bool,

    /// Status message shown briefly in the footer (e.g. "Refreshing…")
    pub status_message: Option<(String, std::time::Instant)>,
}

/// Which panel currently owns keyboard input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The pipeline list on the left
    PipelineList,
    /// The stage details on the right
    Details,
    /// The log viewer (shown when 'l' is pressed)
    Logs,
    /// The error history panel (toggled with 'e')
    Errors,
}

impl App {
    /// Construct the app from a loaded config.  No network calls happen here.
    pub fn new(config: Config) -> Self {
        let refresh_interval = config.refresh_interval;
        Self {
            config,
            state: AppState::new(),
            selected_pipeline: 0,
            focus: Focus::PipelineList,
            logs: None,
            log_stage_index: None,
            log_scroll: 0,
            show_errors: false,
            refresh_interval,
            should_quit: false,
            force_refresh: false,
            status_message: None,
        }
    }

    // -----------------------------------------------------------------------
    // Poll message handling
    // -----------------------------------------------------------------------

    /// Process a single message from the background poller.
    pub fn handle_poll_update(&mut self, update: PollUpdate) {
        match update {
            PollUpdate::PipelinesUpdated(source, pipelines) => {
                self.state.merge_pipelines(&source, pipelines);
                // If the user had a pipeline selected and it shifted, clamp
                self.clamp_selection();
            }
            PollUpdate::Error(source, message) => {
                self.state.mark_source_error(&source, &message);
            }
            PollUpdate::RateLimited(source, duration) => {
                self.state.mark_source_rate_limited(&source, duration);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Keyboard handling
    // -----------------------------------------------------------------------

    /// Route a key-press event to the appropriate handler based on current focus.
    pub fn handle_input(&mut self, key: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;

        // Global keys – always active regardless of focus
        match key {
            KeyCode::Char('q') | KeyCode::Esc => {
                // Esc only quits from top-level panels; from Logs it goes back
                if key == KeyCode::Esc && self.focus == Focus::Logs {
                    self.focus = Focus::Details;
                    self.logs = None;
                    self.log_stage_index = None;
                    return;
                }
                if key == KeyCode::Esc && self.focus == Focus::Errors {
                    self.show_errors = false;
                    self.focus = Focus::PipelineList;
                    return;
                }
                if key == KeyCode::Char('q') {
                    self.should_quit = true;
                }
                return;
            }
            KeyCode::Char('r') => {
                self.force_refresh = true;
                self.set_status("Refreshing…".into());
                return;
            }
            KeyCode::Char('e') => {
                self.show_errors = !self.show_errors;
                if self.show_errors {
                    self.focus = Focus::Errors;
                } else {
                    self.focus = Focus::PipelineList;
                }
                return;
            }
            _ => {}
        }

        // Focus-specific keys
        match self.focus {
            Focus::PipelineList => self.handle_list_input(key),
            Focus::Details => self.handle_details_input(key),
            Focus::Logs => self.handle_logs_input(key),
            Focus::Errors => self.handle_errors_input(key),
        }
    }

    fn handle_list_input(&mut self, key: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match key {
            KeyCode::Up | KeyCode::Char('k') => self.previous_pipeline(),
            KeyCode::Down | KeyCode::Char('j') => self.next_pipeline(),
            KeyCode::Enter => {
                self.focus = Focus::Details;
                // Clear any previously loaded logs when switching pipeline
                self.logs = None;
                self.log_stage_index = None;
            }
            _ => {}
        }
    }

    fn handle_details_input(&mut self, key: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                // Navigate stages – decrement log_stage_index or go back to list
                if let Some(idx) = self.log_stage_index {
                    if idx > 0 {
                        self.log_stage_index = Some(idx - 1);
                    }
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                // Navigate stages – increment log_stage_index
                if let Some(pipeline) = self.selected_pipeline_ref() {
                    let max = pipeline.stages.len().saturating_sub(1);
                    let idx = self.log_stage_index.unwrap_or(0);
                    if idx < max {
                        self.log_stage_index = Some(idx + 1);
                    }
                }
            }
            KeyCode::Char('l') => {
                // Trigger log fetch – mark that we want logs loaded
                // The actual fetch happens in the event loop (it's async)
                self.focus = Focus::Logs;
                self.log_scroll = 0;
                // logs will be fetched by the main loop when it sees focus == Logs && logs == None
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.focus = Focus::PipelineList;
            }
            _ => {}
        }
    }

    fn handle_logs_input(&mut self, key: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match key {
            KeyCode::Up | KeyCode::Char('k') => {
                self.log_scroll = self.log_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let max = self.logs.as_ref().map(|l| l.len()).unwrap_or(0);
                if self.log_scroll < max.saturating_sub(1) {
                    self.log_scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.log_scroll = self.log_scroll.saturating_sub(20);
            }
            KeyCode::PageDown => {
                let max = self.logs.as_ref().map(|l| l.len()).unwrap_or(0);
                self.log_scroll = (self.log_scroll + 20).min(max.saturating_sub(1));
            }
            KeyCode::Home => {
                self.log_scroll = 0;
            }
            KeyCode::End => {
                let max = self.logs.as_ref().map(|l| l.len()).unwrap_or(0);
                self.log_scroll = max.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn handle_errors_input(&mut self, key: crossterm::event::KeyCode) {
        // Errors panel currently has no navigation; 'e' toggles it off (handled globally)
        let _ = key;
    }

    // -----------------------------------------------------------------------
    // Pipeline list navigation
    // -----------------------------------------------------------------------

    fn previous_pipeline(&mut self) {
        let count = self.state.pipeline_count();
        if count == 0 {
            return;
        }
        if self.selected_pipeline > 0 {
            self.selected_pipeline -= 1;
        } else {
            self.selected_pipeline = count - 1;
        }
        self.logs = None;
        self.log_stage_index = None;
    }

    fn next_pipeline(&mut self) {
        let count = self.state.pipeline_count();
        if count == 0 {
            return;
        }
        if self.selected_pipeline < count - 1 {
            self.selected_pipeline += 1;
        } else {
            self.selected_pipeline = 0;
        }
        self.logs = None;
        self.log_stage_index = None;
    }

    /// Ensure `selected_pipeline` doesn't exceed the current list length
    fn clamp_selection(&mut self) {
        let count = self.state.pipeline_count();
        if count == 0 {
            self.selected_pipeline = 0;
        } else if self.selected_pipeline >= count {
            self.selected_pipeline = count - 1;
        }
    }

    // -----------------------------------------------------------------------
    // Selected pipeline accessors
    // -----------------------------------------------------------------------

    /// Borrow the currently selected pipeline, if any
    pub fn selected_pipeline_ref(&self) -> Option<&crate::models::Pipeline> {
        let sorted = self.state.get_sorted_pipelines();
        sorted.get(self.selected_pipeline).copied()
    }

    // -----------------------------------------------------------------------
    // Status message helpers
    // -----------------------------------------------------------------------

    /// Set a transient status message (shown for ~2 seconds in the footer)
    pub fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, std::time::Instant::now()));
    }

    /// Clear the status message if it has been shown for > 2 seconds
    pub fn tick_status(&mut self) {
        if let Some((_, when)) = &self.status_message {
            if when.elapsed().as_secs() > 2 {
                self.status_message = None;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Source status helpers (for the header)
    // -----------------------------------------------------------------------

    /// Collect all source statuses for the header bar
    pub fn source_statuses(&self) -> Vec<(&String, &SourceStatus)> {
        self.state.source_status.iter().collect()
    }
}
