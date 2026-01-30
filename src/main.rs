use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

mod app;
mod config;
mod models;
mod ui;
mod utils;

use app::App;
use config::Config;
use ui::Theme;
use utils::{terminal, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Install panic hook to ensure terminal is restored
    terminal::install_panic_hook();

    // Load configuration
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            eprintln!("Using default configuration...");
            Config::default()
        }
    };

    // Create theme
    let theme = Theme::from_name(&config.ui.theme);

    // Initialize application
    let mut app = App::new(config);
    
    // Initialize with mock data for Phase 1
    app.init_with_mock_data()?;

    // Initialize terminal
    let mut terminal = terminal::init()?;
    let _guard = terminal::TerminalGuard::new();

    // Main event loop
    run_event_loop(&mut terminal, &mut app, &theme).await?;

    // Cleanup is handled by TerminalGuard Drop
    Ok(())
}

/// Main event loop
async fn run_event_loop(
    terminal: &mut terminal::Tui,
    app: &mut App,
    theme: &Theme,
) -> Result<()> {
    loop {
        // Render the UI
        terminal.draw(|f| {
            ui::render(f, app, theme);
        })?;

        // Handle events with a timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events, not release
                if key.kind == KeyEventKind::Press {
                    app.handle_input(key.code);
                }
            }
        }

        // Check if we should quit
        if app.should_quit {
            break;
        }
    }

    Ok(())
}