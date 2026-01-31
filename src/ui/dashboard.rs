use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, Focus};
use crate::ui::theme::Theme;

/// Render the main dashboard layout
pub fn render(f: &mut Frame, app: &App, theme: &Theme) {
    let size = f.size();

    // Create main layout: header, body, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Body
            Constraint::Length(3),  // Footer
        ])
        .split(size);

    // Render header
    render_header(f, chunks[0], app, theme);

    // Render body (pipeline list and details)
    render_body(f, chunks[1], app, theme);

    // Render footer
    render_footer(f, chunks[2], theme);
}

/// Render the header with title and status
fn render_header(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let title = vec![
        Span::styled("ARGUS", theme.title()),
        Span::raw(" - All-Seeing Pipeline Monitor"),
    ];

    let status = if app.pipelines.is_empty() {
        format!(" No pipelines | Refresh: {}s ", app.refresh_interval)
    } else {
        format!(
            " {} pipelines | Refresh: {}s ",
            app.pipelines.len(),
            app.refresh_interval
        )
    };

    let header = Paragraph::new(Line::from(title))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.border())
                .title(status),
        )
        .style(theme.normal());

    f.render_widget(header, area);
}

/// Render the main body with pipeline list and details
fn render_body(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    // Split into two panels: pipeline list (left) and details (right)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),  // Pipeline list
            Constraint::Percentage(50),  // Details
        ])
        .split(area);

    // Render pipeline list
    render_pipeline_list(f, chunks[0], app, theme);

    // Render details panel
    render_details_panel(f, chunks[1], app, theme);
}

/// Render the pipeline list
fn render_pipeline_list(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let is_focused = matches!(app.focus, Focus::PipelineList);
    let border_style = if is_focused {
        theme.border_focused()
    } else {
        theme.border()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Pipelines ");

    if app.pipelines.is_empty() {
        let message = Paragraph::new("No pipelines configured.\n\nAdd pipeline sources in your config file:\n~/.config/argus/config.toml")
            .block(block)
            .style(theme.normal());
        f.render_widget(message, area);
        return;
    }

    // Create pipeline list items
    let items: Vec<Line> = app
        .pipelines
        .iter()
        .enumerate()
        .map(|(i, pipeline)| {
            let style = if i == app.selected_pipeline {
                theme.selected()
            } else {
                theme.normal()
            };

            let status_style = theme.status_style(pipeline.status);
            let status_emoji = pipeline.status.emoji();

            Line::from(vec![
                Span::styled(format!("{} ", status_emoji), status_style),
                Span::styled(&pipeline.name, style),
                Span::styled(format!(" ({})", pipeline.branch), style.add_modifier(Modifier::DIM)),
            ])
        })
        .collect();

    let list = Paragraph::new(items)
        .block(block)
        .style(theme.normal());

    f.render_widget(list, area);
}

/// Render the details panel
fn render_details_panel(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let is_focused = matches!(app.focus, Focus::Details);
    let border_style = if is_focused {
        theme.border_focused()
    } else {
        theme.border()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" Details ");

    if app.pipelines.is_empty() {
        let message = Paragraph::new("Select a pipeline to view details")
            .block(block)
            .style(theme.normal());
        f.render_widget(message, area);
        return;
    }

    // Get selected pipeline
    if let Some(pipeline) = app.pipelines.get(app.selected_pipeline) {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Pipeline: ", theme.highlight()),
                Span::raw(&pipeline.name),
            ]),
            Line::from(vec![
                Span::styled("Status: ", theme.highlight()),
                Span::styled(
                    format!("{} {}", pipeline.status.emoji(), pipeline.status.as_str()),
                    theme.status_style(pipeline.status),
                ),
            ]),
            Line::from(vec![
                Span::styled("Branch: ", theme.highlight()),
                Span::raw(&pipeline.branch),
            ]),
            Line::from(vec![
                Span::styled("Build: ", theme.highlight()),
                Span::raw(format!("#{}", pipeline.build_number)),
            ]),
            Line::from(vec![
                Span::styled("Commit: ", theme.highlight()),
                Span::raw(format!("{} - {}", pipeline.short_commit_sha(), pipeline.short_commit_message())),
            ]),
            Line::from(vec![
                Span::styled("Author: ", theme.highlight()),
                Span::raw(&pipeline.author),
            ]),
            Line::from(""),
            Line::from(Span::styled("Stages:", theme.highlight())),
        ];

        // Add stage information
        for stage in &pipeline.stages {
            let stage_line = Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{} ", stage.status.emoji()),
                    theme.stage_status_style(stage.status),
                ),
                Span::raw(&stage.name),
            ]);
            lines.push(stage_line);
        }

        let details = Paragraph::new(lines)
            .block(block)
            .style(theme.normal());

        f.render_widget(details, area);
    }
}

/// Render the footer with keyboard shortcuts
fn render_footer(f: &mut Frame, area: Rect, theme: &Theme) {
    let help_text = Line::from(vec![
        Span::raw(" "),
        Span::styled("↑/↓", theme.highlight()),
        Span::raw(" Navigate | "),
        Span::styled("Enter", theme.highlight()),
        Span::raw(" Select | "),
        Span::styled("r", theme.highlight()),
        Span::raw(" Refresh | "),
        Span::styled("?", theme.highlight()),
        Span::raw(" Help | "),
        Span::styled("q", theme.highlight()),
        Span::raw(" Quit "),
    ]);

    let footer = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).border_style(theme.border()))
        .style(theme.normal());

    f.render_widget(footer, area);
}