use crossterm::event::{self, Event, KeyEventKind};
use std::time::Duration;

mod api;
mod app;
mod config;
mod models;
mod services;
mod state;
mod ui;
mod utils;

use api::{GitHubClient, CIPlatform};
use app::App;
use config::Config;
use services::PipelinePoller;
use ui::Theme;
use utils::{terminal, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // -----------------------------------------------------------------------
    // 1. Panic hook – ensures the terminal is always restored
    // -----------------------------------------------------------------------
    terminal::install_panic_hook();

    // -----------------------------------------------------------------------
    // 2. Load configuration
    // -----------------------------------------------------------------------
    let cfg = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            eprintln!("Using default configuration…");
            Config::default()
        }
    };

    let theme = Theme::from_name(&cfg.ui.theme);
    let mut app = App::new(cfg.clone());

    // -----------------------------------------------------------------------
    // 3. Build platform clients from configured sources
    // -----------------------------------------------------------------------
    let mut poller = PipelinePoller::new();
    let mut any_source_configured = false;

    for source_cfg in &cfg.sources {
        match source_cfg.source_type.as_str() {
            "github" => {
                match GitHubClient::new(source_cfg) {
                    Ok(client) => {
                        any_source_configured = true;
                        app.state.register_source(&source_cfg.name);

                        // Start background polling for this source
                        let boxed: Box<dyn api::CIPlatform> = Box::new(client);
                        poller.start(boxed, cfg.refresh_interval);
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: failed to initialize GitHub source '{}': {}",
                            source_cfg.name, e
                        );
                        app.state.mark_source_error(&source_cfg.name, &e.to_string());
                    }
                }
            }
            other => {
                eprintln!(
                    "Warning: source type '{}' is not yet supported (Phase 3). Skipping '{}'.",
                    other, source_cfg.name
                );
            }
        }
    }

    // If no sources at all, print a helpful hint before entering the TUI
    if !any_source_configured {
        eprintln!();
        eprintln!("No CI/CD sources configured.");
        eprintln!("Create ~/.config/argus/config.toml with a [[sources]] entry.");
        eprintln!("See the setup guide for details.");
        eprintln!();
    }

    // -----------------------------------------------------------------------
    // 4. Initialize terminal & start event loop
    // -----------------------------------------------------------------------
    let mut terminal = terminal::init()?;
    let _guard = terminal::TerminalGuard::new();

    let mut poll_rx = poller.receiver();

    run_event_loop(&mut terminal, &mut app, &theme, &mut poll_rx, &cfg).await?;

    Ok(())
}

/// Main event loop.
///
/// Each iteration:
///   1. Drain any pending poll messages from the background tasks
///   2. If focus is on Logs and logs haven't been fetched yet, kick off a fetch
///   3. Render the UI
///   4. Wait up to 100 ms for a key event
///   5. If a key arrived, route it through the app
///   6. Tick the status-message timer
async fn run_event_loop(
    terminal: &mut terminal::Tui,
    app: &mut App,
    theme: &Theme,
    poll_rx: &mut tokio::sync::mpsc::Receiver<services::PollUpdate>,
    cfg: &Config,
) -> Result<()> {
    // We need a second tokio handle for spawning log-fetch tasks from inside
    // the loop.  The results come back through a one-shot channel.
    let mut log_result_rx: Option<tokio::sync::oneshot::Receiver<Result<Vec<api::LogEntry>>>> =
        None;

    loop {
        // ------------------------------------------------------------------
        // 1. Drain poll updates (non-blocking)
        // ------------------------------------------------------------------
        while let Ok(update) = poll_rx.try_recv() {
            app.handle_poll_update(update);
        }

        // ------------------------------------------------------------------
        // 1b. Check for completed log fetch
        // ------------------------------------------------------------------
        if let Some(mut rx_taken) = log_result_rx.take() {
            // Try to receive without blocking
            match rx_taken.try_recv() {
                Ok(Ok(entries)) => {
                    app.logs = Some(entries);
                }
                Ok(Err(e)) => {
                    app.logs = Some(vec![]);
                    app.state.mark_source_error("log-fetch", &e.to_string());
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    // Not ready yet, put it back
                    log_result_rx = Some(rx_taken);
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    app.logs = Some(vec![]);
                }
            }
        }

        // ------------------------------------------------------------------
        // 2. Kick off async log fetch if needed
        // ------------------------------------------------------------------
        if app.focus == app::Focus::Logs && app.logs.is_none() && log_result_rx.is_none() {
            // Find the pipeline and stage we need logs for
            if let Some(pipeline) = app.selected_pipeline_ref() {
                let pipeline_id = pipeline.id.clone();
                let stage_idx = app.log_stage_index.unwrap_or(0);

                if let Some(stage) = pipeline.stages.get(stage_idx) {
                    let stage_id = stage.id.clone();

                    // We need to re-create a client for this async task.
                    // In the future we'd match by source and clone the right one.
                    // For now, Phase 2 only has GitHub, so we rebuild from config.
                    if let Some(source_cfg) = cfg.sources.first() {
                        if let Ok(client) = api::GitHubClient::new(source_cfg) {
                            let (tx, rx) = tokio::sync::oneshot::channel();
                            log_result_rx = Some(rx);

                            tokio::spawn(async move {
                                let result = client.fetch_logs(&pipeline_id, &stage_id).await;
                                let _ = tx.send(result);
                            });
                        } else {
                            app.logs = Some(vec![]);
                        }
                    } else {
                        app.logs = Some(vec![]);
                    }
                } else {
                    app.logs = Some(vec![]);
                }
            } else {
                app.logs = Some(vec![]);
            }
        }

        // ------------------------------------------------------------------
        // 3. Render
        // ------------------------------------------------------------------
        terminal.draw(|f| {
            ui::render(f, app, theme);
        })?;

        // ------------------------------------------------------------------
        // 4. Wait for input (100 ms timeout keeps the loop responsive)
        // ------------------------------------------------------------------
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_input(key.code);
                }
            }
        }

        // ------------------------------------------------------------------
        // 5. Tick status message timer
        // ------------------------------------------------------------------
        app.tick_status();

        // ------------------------------------------------------------------
        // 6. Quit?
        // ------------------------------------------------------------------
        if app.should_quit {
            break;
        }
    }

    Ok(())
}