use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};

use crate::utils::error::{Result, UiError};

/// Terminal type alias
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal for TUI mode
pub fn init() -> Result<Tui> {
    // Enable raw mode for input handling
    enable_raw_mode()
        .map_err(|e| UiError::InitFailed(format!("Failed to enable raw mode: {}", e)))?;

    // Enter alternate screen to preserve terminal state
    execute!(io::stdout(), EnterAlternateScreen)
        .map_err(|e| UiError::InitFailed(format!("Failed to enter alternate screen: {}", e)))?;

    // Create the terminal backend
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)
        .map_err(|e| UiError::InitFailed(format!("Failed to create terminal: {}", e)))?;

    Ok(terminal)
}

/// Restore the terminal to its original state
pub fn restore() -> Result<()> {
    // Leave alternate screen
    execute!(io::stdout(), LeaveAlternateScreen)
        .map_err(|e| UiError::Terminal(format!("Failed to leave alternate screen: {}", e)))?;

    // Disable raw mode
    disable_raw_mode()
        .map_err(|e| UiError::Terminal(format!("Failed to disable raw mode: {}", e)))?;

    Ok(())
}

/// Install a panic hook to ensure terminal is restored on panic
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        // Try to restore terminal
        let _ = restore();

        // Call the original panic hook
        original_hook(panic_info);
    }));
}

/// RAII guard for terminal cleanup
pub struct TerminalGuard;

impl TerminalGuard {
    pub fn new() -> Self {
        install_panic_hook();
        Self
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = restore();
    }
}
