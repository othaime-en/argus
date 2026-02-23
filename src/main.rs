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

use api::{GitHubClient, GitLabClient, JenkinsClient};
use app::App;
use config::Config;
use services::PipelinePoller;
use ui::Theme;
use utils::{terminal, Result};

#[tokio::main]
async fn main() -> Result<()> {
    terminal::install_panic_hook();

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

    let mut poller = PipelinePoller::new();
    let mut any_source_configured = false;

    for source_cfg in cfg.active_sources() {
        let client: Box<dyn api::CIPlatform> = match source_cfg.source_type.as_str() {
            "github" => match GitHubClient::new(source_cfg) {
                Ok(c) => Box::new(c),
                Err(e) => {
                    eprintln!(
                        "Warning: failed to initialize GitHub source '{}': {}",
                        source_cfg.name, e
                    );
                    app.state.mark_source_error(&source_cfg.name, &e.to_string());
                    continue;
                }
            },
            "gitlab" => match GitLabClient::new(source_cfg) {
                Ok(c) => Box::new(c),
                Err(e) => {
                    eprintln!(
                        "Warning: failed to initialize GitLab source '{}': {}",
                        source_cfg.name, e
                    );
                    app.state.mark_source_error(&source_cfg.name, &e.to_string());
                    continue;
                }
            },
            "jenkins" => match JenkinsClient::new(source_cfg) {
                Ok(c) => Box::new(c),
                Err(e) => {
                    eprintln!(
                        "Warning: failed to initialize Jenkins source '{}': {}",
                        source_cfg.name, e
                    );
                    app.state.mark_source_error(&source_cfg.name, &e.to_string());
                    continue;
                }
            },
            other => {
                eprintln!(
                    "Warning: unsupported source type '{}' in source '{}'. Skipping.",
                    other, source_cfg.name
                );
                continue;
            }
        };

        any_source_configured = true;
        app.state.register_source(&source_cfg.name);
        poller.start(client, cfg.refresh_interval);
    }

    if !any_source_configured {
        eprintln!();
        eprintln!("No CI/CD sources configured.");
        eprintln!("Create ~/.config/argus/config.toml with a [[sources]] entry.");
        eprintln!("See config/example.toml for a complete reference.");
        eprintln!();
    }

    let mut terminal = terminal::init()?;
    let _guard = terminal::TerminalGuard::new();

    let mut poll_rx = poller.receiver();
    run_event_loop(&mut terminal, &mut app, &theme, &mut poll_rx, &cfg).await?;

    Ok(())
}

async fn run_event_loop(
    terminal: &mut terminal::Tui,
    app: &mut App,
    theme: &Theme,
    poll_rx: &mut tokio::sync::mpsc::Receiver<services::PollUpdate>,
    cfg: &Config,
) -> Result<()> {
    let mut log_result_rx: Option<tokio::sync::oneshot::Receiver<Result<Vec<api::LogEntry>>>> =
        None;

    loop {
        // Drain poll updates
        while let Ok(update) = poll_rx.try_recv() {
            app.handle_poll_update(update);
        }

        // Check for a completed log fetch
        if let Some(mut rx_taken) = log_result_rx.take() {
            match rx_taken.try_recv() {
                Ok(Ok(entries)) => {
                    app.logs = Some(entries);
                }
                Ok(Err(e)) => {
                    app.logs = Some(vec![]);
                    app.state.mark_source_error("log-fetch", &e.to_string());
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                    log_result_rx = Some(rx_taken);
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    app.logs = Some(vec![]);
                }
            }
        }

        // Kick off async log fetch if needed
        if app.focus == app::Focus::Logs && app.logs.is_none() && log_result_rx.is_none() {
            if let Some(pipeline) = app.selected_pipeline_ref() {
                let pipeline_id = pipeline.id.clone();
                let stage_idx = app.log_stage_index.unwrap_or(0);

                if let Some(stage) = pipeline.stages.get(stage_idx) {
                    let stage_id = stage.id.clone();

                    // Determine which source config matches this pipeline's source prefix
                    let source_cfg = cfg.active_sources().find(|sc| {
                        pipeline_id.starts_with(&format!("{}-", sc.source_type))
                            || pipeline.source.to_lowercase().contains(&sc.source_type)
                    });

                    let client_result: Option<Box<dyn api::CIPlatform>> =
                        if let Some(sc) = source_cfg {
                            match sc.source_type.as_str() {
                                "github" => {
                                    GitHubClient::new(sc).ok().map(|c| Box::new(c) as Box<dyn api::CIPlatform>)
                                }
                                "gitlab" => {
                                    GitLabClient::new(sc).ok().map(|c| Box::new(c) as Box<dyn api::CIPlatform>)
                                }
                                "jenkins" => {
                                    JenkinsClient::new(sc).ok().map(|c| Box::new(c) as Box<dyn api::CIPlatform>)
                                }
                                _ => None,
                            }
                        } else {
                            None
                        };

                    if let Some(client) = client_result {
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
        }

        terminal.draw(|f| {
            ui::render(f, app, theme);
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_input(key.code);
                }
            }
        }

        app.tick_status();

        if app.should_quit {
            break;
        }
    }

    Ok(())
}