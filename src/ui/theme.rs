use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Debug)]
pub struct Theme {
    pub border: Style,
    pub border_focused: Style,
    pub title: Style,
    pub header: Style,

    // Gauge / bar fills
    pub gauge_low: Style,     // 0–60%
    pub gauge_medium: Style,  // 60–85%
    pub gauge_high: Style,    // 85–100%

    // Sparkline
    pub sparkline: Style,

    // Process table
    pub row_normal: Style,
    pub row_selected: Style,
    pub row_thread: Style,
    pub row_suspicious: Style,

    // Status text
    pub status_running: Style,
    pub status_suspended: Style,
    pub status_other: Style,

    // Filter bar
    pub filter_active: Style,
    pub filter_inactive: Style,

    pub help_bg: Style,
    pub text_dim: Style,
    pub text_normal: Style,
    pub text_bright: Style,
}

impl Theme {
    pub fn default_dark() -> Self {
        Self {
            border: Style::default().fg(Color::DarkGray),
            border_focused: Style::default().fg(Color::Cyan),
            title: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            header: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),

            gauge_low: Style::default().fg(Color::Green),
            gauge_medium: Style::default().fg(Color::Yellow),
            gauge_high: Style::default().fg(Color::Red),

            sparkline: Style::default().fg(Color::Green),

            row_normal: Style::default().fg(Color::White),
            row_selected: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            row_thread: Style::default().fg(Color::DarkGray),
            row_suspicious: Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),

            status_running: Style::default().fg(Color::Green),
            status_suspended: Style::default().fg(Color::Yellow),
            status_other: Style::default().fg(Color::Gray),

            filter_active: Style::default().fg(Color::Black).bg(Color::Yellow),
            filter_inactive: Style::default().fg(Color::DarkGray),

            help_bg: Style::default().bg(Color::DarkGray),
            text_dim: Style::default().fg(Color::DarkGray),
            text_normal: Style::default().fg(Color::White),
            text_bright: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        }
    }

    /// Pick a gauge style based on percentage (0–100).
    pub fn gauge_for_pct(&self, pct: f64) -> Style {
        if pct >= 85.0 {
            self.gauge_high
        } else if pct >= 60.0 {
            self.gauge_medium
        } else {
            self.gauge_low
        }
    }
}
