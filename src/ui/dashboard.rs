use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, Focus};
use crate::state::SourceStatus;
use crate::ui::theme::Theme;
use crate::utils::time::{format_duration_compact, format_relative};

pub fn render(f: &mut Frame, app: &App, theme: &Theme) {
    let size = f.size();

    if app.show_errors {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(8),
                Constraint::Length(3),
            ])
            .split(size);

        render_header(f, chunks[0], app, theme);
        render_footer(f, chunks[3], app, theme);
        render_error_panel(f, chunks[2], app, theme);
        render_body(f, chunks[1], app, theme);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(size);

        render_header(f, chunks[0], app, theme);
        render_footer(f, chunks[2], app, theme);
        render_body(f, chunks[1], app, theme);
    }
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

/// Return a distinct color for each known platform type so the source legend
/// is immediately recognisable (GitHub=Blue, GitLab=Orange/Yellow, Jenkins=Red).
fn source_type_color(source_name: &str, status: &SourceStatus) -> Style {
    // When there is a hard error or rate-limit we let the status colour win so
    // the operator immediately notices the problem.
    match status {
        SourceStatus::Error(_) => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        SourceStatus::RateLimited(_) => Style::default().fg(Color::Yellow),
        _ => {
            // Infer platform from the source name; callers embed the type name.
            let lc = source_name.to_lowercase();
            if lc.contains("github") {
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            } else if lc.contains("gitlab") {
                // GitLab brand colour is roughly #FC6D26 (orange).
                Style::default().fg(Color::Rgb(252, 109, 38)).add_modifier(Modifier::BOLD)
            } else if lc.contains("jenkins") {
                // Jenkins brand colour is a muted red/brown.
                Style::default().fg(Color::Rgb(215, 58, 74)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            }
        }
    }
}

fn render_header(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let mut badge_spans: Vec<Span> = Vec::new();
    badge_spans.push(Span::raw("Sources: "));

    for (name, status) in app.source_statuses() {
        let (icon, conn_label) = match status {
            SourceStatus::Connected => ("✓", ""),
            SourceStatus::Connecting => ("⟳", ""),
            SourceStatus::Error(_) => ("✗", ""),
            SourceStatus::RateLimited(_) => ("⏳", ""),
        };

        let style = source_type_color(name, status);
        badge_spans.push(Span::styled(format!("[{} {} {}] ", name, icon, conn_label), style));
    }

    let title_line = Line::from(vec![
        Span::styled("ARGUS", theme.title()),
        Span::raw(" ─ All-Seeing Pipeline Monitor"),
    ]);

    let status_text = format!(
        " {} pipelines | {}s refresh ",
        app.state.pipeline_count(),
        app.refresh_interval
    );

    let header = Paragraph::new(title_line)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border())
                .title(status_text),
        )
        .style(theme.normal());

    f.render_widget(header, area);

    if !badge_spans.is_empty() && area.height > 2 {
        let badge_area = Rect {
            x: area.x + 1,
            y: area.y + 1,
            width: area.width.saturating_sub(2),
            height: 1,
        };
        f.render_widget(Paragraph::new(Line::from(badge_spans)), badge_area);
    }
}

// ---------------------------------------------------------------------------
// Body
// ---------------------------------------------------------------------------

fn render_body(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    render_pipeline_list(f, chunks[0], app, theme);

    if app.focus == Focus::Logs {
        render_log_viewer(f, chunks[1], app, theme);
    } else {
        render_details_panel(f, chunks[1], app, theme);
    }
}

// ---------------------------------------------------------------------------
// Pipeline list – shows a source-type badge next to each entry
// ---------------------------------------------------------------------------

/// Short platform label shown as a coloured prefix in the pipeline list.
fn platform_badge(source: &str) -> (&'static str, Color) {
    let lc = source.to_lowercase();
    if lc.contains("github") {
        ("GH", Color::Blue)
    } else if lc.contains("gitlab") {
        ("GL", Color::Rgb(252, 109, 38))
    } else if lc.contains("jenkins") {
        ("JK", Color::Rgb(215, 58, 74))
    } else {
        ("CI", Color::Cyan)
    }
}

fn render_pipeline_list(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let is_focused = app.focus == Focus::PipelineList;
    let border_style = if is_focused { theme.border_focused() } else { theme.border() };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Pipelines ");

    let pipelines = app.state.get_sorted_pipelines();

    if pipelines.is_empty() {
        let msg = if app.state.source_status.is_empty() {
            "No sources configured.\n\nAdd [[sources]] to ~/.config/argus/config.toml"
        } else {
            "Fetching pipelines…\n\nNo data yet. Press 'r' to force refresh."
        };
        f.render_widget(Paragraph::new(msg).block(block).style(theme.normal()), area);
        return;
    }

    let visible_rows = (area.height as usize).saturating_sub(2);
    let scroll_offset = if app.selected_pipeline >= visible_rows {
        app.selected_pipeline - visible_rows + 1
    } else {
        0
    };

    let lines: Vec<Line> = pipelines
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_rows)
        .map(|(i, pipeline)| {
            let is_selected = i == app.selected_pipeline;
            let base_style = if is_selected { theme.selected() } else { theme.normal() };
            let status_style = theme.status_style(pipeline.status);

            let (badge_label, badge_color) = platform_badge(&pipeline.source);
            let badge_style = if is_selected {
                // Keep badge visible on selected background
                Style::default().fg(badge_color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(badge_color).add_modifier(Modifier::BOLD)
            };

            let max_name = 20usize;
            let name_display = if pipeline.name.len() > max_name {
                format!("{}…", &pipeline.name[..max_name - 1])
            } else {
                pipeline.name.clone()
            };

            let dur_str = pipeline
                .duration
                .map(|d| format!(" {}", format_duration_compact(&d)))
                .unwrap_or_default();

            Line::from(vec![
                Span::styled(format!("[{}] ", badge_label), badge_style),
                Span::styled(format!("{} ", pipeline.status.emoji()), status_style),
                Span::styled(name_display, base_style),
                Span::styled(
                    format!(" ({})", pipeline.branch),
                    base_style.add_modifier(Modifier::DIM),
                ),
                Span::styled(dur_str, base_style.add_modifier(Modifier::DIM)),
            ])
        })
        .collect();

    let total = pipelines.len();
    let scroll_text = if total > visible_rows {
        format!(" {}/{} ", app.selected_pipeline + 1, total)
    } else {
        String::new()
    };

    let block = if !scroll_text.is_empty() {
        block.title(ratatui::widgets::block::Title::from(scroll_text).alignment(Alignment::Right))
    } else {
        block
    };

    f.render_widget(Paragraph::new(lines).block(block).style(theme.normal()), area);
}

// ---------------------------------------------------------------------------
// Details panel
// ---------------------------------------------------------------------------

fn render_details_panel(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let is_focused = app.focus == Focus::Details;
    let border_style = if is_focused { theme.border_focused() } else { theme.border() };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Details ");

    let pipeline = match app.selected_pipeline_ref() {
        Some(p) => p,
        None => {
            f.render_widget(
                Paragraph::new("No pipelines available").block(block).style(theme.normal()),
                area,
            );
            return;
        }
    };

    let (badge_label, badge_color) = platform_badge(&pipeline.source);
    let badge_style = Style::default().fg(badge_color).add_modifier(Modifier::BOLD);

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("Platform:  ", theme.highlight()),
            Span::styled(format!("[{}] {}", badge_label, pipeline.source), badge_style),
        ]),
        Line::from(vec![
            Span::styled("Pipeline:  ", theme.highlight()),
            Span::raw(&pipeline.name),
        ]),
        Line::from(vec![
            Span::styled("Repository:", theme.highlight()),
            Span::raw(format!(" {}", pipeline.repository)),
        ]),
        Line::from(vec![
            Span::styled("Status:    ", theme.highlight()),
            Span::styled(
                format!("{} {}", pipeline.status.emoji(), pipeline.status.as_str()),
                theme.status_style(pipeline.status),
            ),
        ]),
        Line::from(vec![
            Span::styled("Branch:    ", theme.highlight()),
            Span::raw(&pipeline.branch),
        ]),
        Line::from(vec![
            Span::styled("Build:     ", theme.highlight()),
            Span::raw(format!("#{}", pipeline.build_number)),
        ]),
        Line::from(vec![
            Span::styled("Commit:    ", theme.highlight()),
            Span::raw(format!(
                "{} – {}",
                pipeline.short_commit_sha(),
                pipeline.short_commit_message()
            )),
        ]),
        Line::from(vec![
            Span::styled("Author:    ", theme.highlight()),
            Span::raw(&pipeline.author),
        ]),
        Line::from(vec![
            Span::styled("Started:   ", theme.highlight()),
            Span::raw(format_relative(&pipeline.started_at)),
        ]),
    ];

    if let Some(dur) = &pipeline.duration {
        lines.push(Line::from(vec![
            Span::styled("Duration:  ", theme.highlight()),
            Span::raw(format_duration_compact(dur)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Stages:  (↑↓ to select, 'l' for logs)",
        theme.highlight(),
    )));
    lines.push(Line::from(""));

    let selected_stage = app.log_stage_index.unwrap_or(0);
    for (i, stage) in pipeline.stages.iter().enumerate() {
        let is_stage_selected = i == selected_stage && is_focused;
        let stage_style = if is_stage_selected { theme.selected() } else { theme.normal() };
        let status_style = theme.stage_status_style(stage.status);
        let prefix = if is_stage_selected { " ▶ " } else { "   " };
        let dur_text = stage
            .duration
            .map(|d| format!(" ({})", format_duration_compact(&d)))
            .unwrap_or_default();

        lines.push(Line::from(vec![
            Span::styled(prefix, stage_style),
            Span::styled(format!("{} ", stage.status.emoji()), status_style),
            Span::styled(&stage.name, stage_style),
            Span::styled(dur_text, stage_style.add_modifier(Modifier::DIM)),
        ]));
    }

    f.render_widget(Paragraph::new(lines).block(block).style(theme.normal()), area);
}

// ---------------------------------------------------------------------------
// Log viewer
// ---------------------------------------------------------------------------

fn render_log_viewer(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border_focused())
        .title(" Logs  (↑↓ PgUp PgDn Home End | Esc to close) ");

    match &app.logs {
        None => {
            f.render_widget(
                Paragraph::new("  Fetching logs…").block(block).style(theme.normal()),
                area,
            );
        }
        Some(entries) if entries.is_empty() => {
            f.render_widget(
                Paragraph::new("  No log output available for this stage.")
                    .block(block)
                    .style(theme.normal()),
                area,
            );
        }
        Some(entries) => {
            let visible_rows = (area.height as usize).saturating_sub(2);
            let scroll = app.log_scroll;

            let lines: Vec<Line> = entries
                .iter()
                .skip(scroll)
                .take(visible_rows)
                .map(|entry| {
                    let text_lower = entry.text.to_lowercase();
                    let style = if text_lower.contains("error")
                        || text_lower.contains("failed")
                        || text_lower.contains("fatal")
                    {
                        theme.status_style(crate::models::PipelineStatus::Failed)
                    } else if text_lower.contains("warning") || text_lower.contains("warn") {
                        theme.status_style(crate::models::PipelineStatus::Running)
                    } else if text_lower.contains("success")
                        || text_lower.contains("passed")
                        || text_lower.contains("✓")
                    {
                        theme.status_style(crate::models::PipelineStatus::Success)
                    } else {
                        theme.normal()
                    };

                    let line_num = format!("{:>5} ", entry.line);
                    Line::from(vec![
                        Span::styled(line_num, theme.normal().add_modifier(Modifier::DIM)),
                        Span::styled(&entry.text, style),
                    ])
                })
                .collect();

            let total = entries.len();
            let title_text = format!(" {}/{} ", scroll + 1, total);
            let block = block.title(
                ratatui::widgets::block::Title::from(title_text).alignment(Alignment::Right),
            );

            f.render_widget(Paragraph::new(lines).block(block).style(theme.normal()), area);
        }
    }
}

// ---------------------------------------------------------------------------
// Error panel
// ---------------------------------------------------------------------------

fn render_error_panel(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.status_style(crate::models::PipelineStatus::Failed))
        .title(" Errors  ('e' to close) ");

    let errors = &app.state.errors;
    if errors.is_empty() {
        f.render_widget(
            Paragraph::new("  No recent errors.").block(block).style(theme.normal()),
            area,
        );
        return;
    }

    let visible = (area.height as usize).saturating_sub(2);
    let lines: Vec<Line> = errors
        .iter()
        .rev()
        .take(visible)
        .map(|err| {
            Line::from(vec![
                Span::styled(
                    format!(" {} ", format_relative(&err.timestamp)),
                    theme.normal().add_modifier(Modifier::DIM),
                ),
                Span::styled(
                    format!("[{}] ", err.source),
                    theme.status_style(crate::models::PipelineStatus::Running),
                ),
                Span::styled(
                    &err.message,
                    theme.status_style(crate::models::PipelineStatus::Failed),
                ),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines).block(block).style(theme.normal()), area);
}

// ---------------------------------------------------------------------------
// Footer
// ---------------------------------------------------------------------------

fn render_footer(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let content = if let Some((msg, _)) = &app.status_message {
        Line::from(vec![Span::raw(" "), Span::styled(msg, theme.highlight())])
    } else {
        match app.focus {
            Focus::PipelineList => Line::from(vec![
                Span::raw(" "),
                Span::styled("↑↓", theme.highlight()),
                Span::raw(" Navigate | "),
                Span::styled("Enter", theme.highlight()),
                Span::raw(" Details | "),
                Span::styled("r", theme.highlight()),
                Span::raw(" Refresh | "),
                Span::styled("e", theme.highlight()),
                Span::raw(" Errors | "),
                Span::styled("q", theme.highlight()),
                Span::raw(" Quit"),
            ]),
            Focus::Details => Line::from(vec![
                Span::raw(" "),
                Span::styled("↑↓", theme.highlight()),
                Span::raw(" Stages | "),
                Span::styled("l", theme.highlight()),
                Span::raw(" Logs | "),
                Span::styled("←", theme.highlight()),
                Span::raw(" Back | "),
                Span::styled("e", theme.highlight()),
                Span::raw(" Errors | "),
                Span::styled("q", theme.highlight()),
                Span::raw(" Quit"),
            ]),
            Focus::Logs => Line::from(vec![
                Span::raw(" "),
                Span::styled("↑↓ PgUp PgDn", theme.highlight()),
                Span::raw(" Scroll | "),
                Span::styled("Home End", theme.highlight()),
                Span::raw(" Jump | "),
                Span::styled("Esc", theme.highlight()),
                Span::raw(" Close"),
            ]),
            Focus::Errors => Line::from(vec![
                Span::raw(" "),
                Span::styled("e", theme.highlight()),
                Span::raw(" Close errors panel"),
            ]),
        }
    };

    f.render_widget(
        Paragraph::new(content).block(
            Block::default().borders(Borders::ALL).border_style(theme.border()),
        ).style(theme.normal()),
        area,
    );
}