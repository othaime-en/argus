use ratatui::style::{Color, Modifier, Style};

use crate::models::{PipelineStatus, StageStatus};

/// Theme configuration for the UI
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
    pub border_focused: Color,
    pub title: Color,
    pub success: Color,
    pub running: Color,
    pub failed: Color,
    pub pending: Color,
    pub cancelled: Color,
    pub skipped: Color,
    pub selected: Color,
    pub highlight: Color,
}

impl Theme {
    /// Get the default theme
    pub fn default() -> Self {
        Self {
            name: "default".to_string(),
            background: Color::Reset,
            foreground: Color::Reset,
            border: Color::Gray,
            border_focused: Color::Cyan,
            title: Color::Cyan,
            success: Color::Green,
            running: Color::Yellow,
            failed: Color::Red,
            pending: Color::Gray,
            cancelled: Color::Magenta,
            skipped: Color::DarkGray,
            selected: Color::Blue,
            highlight: Color::Cyan,
        }
    }

    /// Get the dark theme
    pub fn dark() -> Self {
        Self {
            name: "dark".to_string(),
            background: Color::Black,
            foreground: Color::White,
            border: Color::DarkGray,
            border_focused: Color::Cyan,
            title: Color::Cyan,
            success: Color::Green,
            running: Color::Yellow,
            failed: Color::Red,
            pending: Color::Gray,
            cancelled: Color::Magenta,
            skipped: Color::DarkGray,
            selected: Color::Blue,
            highlight: Color::Cyan,
        }
    }

    /// Get the light theme
    pub fn light() -> Self {
        Self {
            name: "light".to_string(),
            background: Color::White,
            foreground: Color::Black,
            border: Color::Gray,
            border_focused: Color::Blue,
            title: Color::Blue,
            success: Color::Green,
            running: Color::Yellow,
            failed: Color::Red,
            pending: Color::Gray,
            cancelled: Color::Magenta,
            skipped: Color::DarkGray,
            selected: Color::Cyan,
            highlight: Color::Blue,
        }
    }

    /// Get the monokai theme
    pub fn monokai() -> Self {
        Self {
            name: "monokai".to_string(),
            background: Color::Rgb(39, 40, 34),
            foreground: Color::Rgb(248, 248, 242),
            border: Color::Rgb(117, 113, 94),
            border_focused: Color::Rgb(102, 217, 239),
            title: Color::Rgb(102, 217, 239),
            success: Color::Rgb(166, 226, 46),
            running: Color::Rgb(253, 151, 31),
            failed: Color::Rgb(249, 38, 114),
            pending: Color::Rgb(117, 113, 94),
            cancelled: Color::Rgb(174, 129, 255),
            skipped: Color::Rgb(117, 113, 94),
            selected: Color::Rgb(73, 72, 62),
            highlight: Color::Rgb(102, 217, 239),
        }
    }

    /// Load a theme by name
    pub fn from_name(name: &str) -> Self {
        match name {
            "dark" => Self::dark(),
            "light" => Self::light(),
            "monokai" => Self::monokai(),
            _ => Self::default(),
        }
    }

    /// Get the color for a pipeline status
    pub fn status_color(&self, status: PipelineStatus) -> Color {
        match status {
            PipelineStatus::Success => self.success,
            PipelineStatus::Running => self.running,
            PipelineStatus::Failed => self.failed,
            PipelineStatus::Pending => self.pending,
            PipelineStatus::Cancelled => self.cancelled,
            PipelineStatus::Skipped => self.skipped,
        }
    }

    /// Get the color for a stage status
    pub fn stage_status_color(&self, status: StageStatus) -> Color {
        match status {
            StageStatus::Success => self.success,
            StageStatus::Running => self.running,
            StageStatus::Failed => self.failed,
            StageStatus::Pending => self.pending,
            StageStatus::Skipped => self.skipped,
        }
    }

    /// Get a style for normal text
    pub fn normal(&self) -> Style {
        Style::default().fg(self.foreground).bg(self.background)
    }

    /// Get a style for titles
    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.title)
            .bg(self.background)
            .add_modifier(Modifier::BOLD)
    }

    /// Get a style for borders
    pub fn border(&self) -> Style {
        Style::default().fg(self.border)
    }

    /// Get a style for focused borders
    pub fn border_focused(&self) -> Style {
        Style::default().fg(self.border_focused)
    }

    /// Get a style for selected items
    pub fn selected(&self) -> Style {
        Style::default()
            .bg(self.selected)
            .fg(self.foreground)
            .add_modifier(Modifier::BOLD)
    }

    /// Get a style for highlighted text
    pub fn highlight(&self) -> Style {
        Style::default()
            .fg(self.highlight)
            .add_modifier(Modifier::BOLD)
    }

    /// Get a style for a pipeline status
    pub fn status_style(&self, status: PipelineStatus) -> Style {
        Style::default()
            .fg(self.status_color(status))
            .add_modifier(Modifier::BOLD)
    }

    /// Get a style for a stage status
    pub fn stage_status_style(&self, status: StageStatus) -> Style {
        Style::default()
            .fg(self.stage_status_color(status))
            .add_modifier(Modifier::BOLD)
    }
}
