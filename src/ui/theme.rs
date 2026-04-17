use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;

use crate::config::GaugeStyle;

/// Unicode sub-cell block chars used in the CPU sparkline.
pub const SPARK_CHARS: &[&str] = &[" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
/// Pure-ASCII fallback sparkline chars for no-color / minimal terminals.
pub const ASCII_SPARK_CHARS: &[&str] = &[" ", ".", "-", "-", "=", "=", "+", "#", "#"];

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

    // Process table
    pub row_normal: Style,
    pub row_zebra: Style,      // subtle alternate background for odd rows
    pub row_selected: Style,
    pub row_thread: Style,
    pub row_suspicious: Style,
    /// Brief flash style for rows whose CPU% jumped >15 pp since last sample.
    pub row_spike: Style,

    // Status text
    pub status_running: Style,
    pub status_suspended: Style,
    pub status_other: Style,

    // Filter bar
    pub filter_active: Style,
    pub filter_inactive: Style,

    pub text_dim: Style,
    pub text_normal: Style,
    pub text_bright: Style,

    /// Background applied to overlay panels and unset table rows.
    /// Set to an explicit white on light themes so black text is always visible
    /// regardless of the terminal's default background colour.
    pub panel_bg: Style,

    /// Box-drawing characters used for all panel borders.
    pub border_set: symbols::border::Set,
    /// How CPU / memory gauge bars are rendered.
    pub gauge_style: GaugeStyle,

    /// Sparkline gradient — low / mid / high fill colours.
    pub spark_low:  Color,
    pub spark_mid:  Color,
    pub spark_high: Color,
    /// Block characters for the sparkline (Unicode or ASCII fallback).
    pub spark_chars: &'static [&'static str],
}

impl Theme {
    /// Gradient colour for the sparkline bar at the given percentage (0–100).
    pub fn spark_color(&self, pct: f64) -> Color {
        if pct >= 85.0 { self.spark_high }
        else if pct >= 60.0 { self.spark_mid }
        else { self.spark_low }
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

    // ── Compiled-in fallback ──────────────────────────────────────────────────

    pub fn default_dark() -> Self {
        Self {
            border:         Style::default().fg(Color::DarkGray),
            border_focused: Style::default().fg(Color::Cyan),
            title:          Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            header:         Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),

            gauge_low:    Style::default().fg(Color::Green),
            gauge_medium: Style::default().fg(Color::Yellow),
            gauge_high:   Style::default().fg(Color::Red),

            row_normal:    Style::default().fg(Color::White),
            row_zebra:     Style::default().fg(Color::White).bg(Color::Rgb(22, 22, 32)),
            row_selected:  Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
            row_thread:    Style::default().fg(Color::DarkGray),
            row_suspicious: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            row_spike:     Style::default().fg(Color::Rgb(255, 200, 0)).add_modifier(Modifier::BOLD),

            status_running:   Style::default().fg(Color::Green),
            status_suspended: Style::default().fg(Color::Yellow),
            status_other:     Style::default().fg(Color::Gray),

            filter_active:   Style::default().fg(Color::Black).bg(Color::Yellow),
            filter_inactive: Style::default().fg(Color::DarkGray),

            text_dim:    Style::default().fg(Color::DarkGray),
            text_normal: Style::default().fg(Color::White),
            text_bright: Style::default().fg(Color::White).add_modifier(Modifier::BOLD),

            panel_bg:   Style::default(),
            border_set: symbols::border::PLAIN,
            gauge_style: GaugeStyle::Block,
            spark_low:  Color::Green,
            spark_mid:  Color::Yellow,
            spark_high: Color::Red,
            spark_chars: SPARK_CHARS,
        }
    }

    /// Monochrome fallback when NO_COLOR is set — no colour at all.
    pub fn no_color() -> Self {
        let normal = Style::default();
        let bold   = Style::default().add_modifier(Modifier::BOLD);
        let dim    = Style::default().add_modifier(Modifier::DIM);
        let invert = Style::default().add_modifier(Modifier::REVERSED);

        Self {
            border:         normal,
            border_focused: bold,
            title:          bold,
            header:         bold,

            gauge_low:    normal,
            gauge_medium: normal,
            gauge_high:   bold,

            row_normal:    normal,
            row_zebra:     normal,
            row_selected:  invert,
            row_thread:    dim,
            row_suspicious: bold,
            row_spike:     bold,

            status_running:   normal,
            status_suspended: dim,
            status_other:     dim,

            filter_active:   invert,
            filter_inactive: dim,

            text_dim:    dim,
            text_normal: normal,
            text_bright: bold,

            border_set: symbols::border::Set {
                top_left:          "+",
                top_right:         "+",
                bottom_left:       "+",
                bottom_right:      "+",
                vertical_left:     "|",
                vertical_right:    "|",
                horizontal_top:    "-",
                horizontal_bottom: "-",
            },
            panel_bg:    Style::default(),
            gauge_style: GaugeStyle::Ascii,
            spark_low:   Color::Reset,
            spark_mid:   Color::Reset,
            spark_high:  Color::Reset,
            spark_chars: ASCII_SPARK_CHARS,
        }
    }
}
