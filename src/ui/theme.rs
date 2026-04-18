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

    /// Semantic danger/error style - used for kill confirm, error messages.
    pub gauge_high: Style,

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

    /// Block characters for the sparkline (Unicode or ASCII fallback).
    pub spark_chars: &'static [&'static str],
}

/// 8-band heat gradient: dark blue (idle) → deep red/crimson (max load).
///
/// Bands (each ~12.5 pp):
///   0–12 %  Dark Blue     13–25 %  Cyan/Light Blue
///  26–37 %  Green         38–50 %  Yellow-Green
///  51–62 %  Yellow/Gold   63–75 %  Orange
///  76–87 %  Red-Orange    88–100 % Deep Red/Crimson
pub fn heat_color(pct: f64) -> Color {
    match pct as u8 {
        0..=12  => Color::Rgb(  0,  80, 200),
        13..=25 => Color::Rgb(  0, 190, 230),
        26..=37 => Color::Rgb(  0, 200, 100),
        38..=50 => Color::Rgb(140, 220,   0),
        51..=62 => Color::Rgb(255, 220,   0),
        63..=75 => Color::Rgb(255, 140,   0),
        76..=87 => Color::Rgb(255,  50,   0),
        _       => Color::Rgb(180,   0,  30),
    }
}

impl Theme {
    /// Gradient colour for the sparkline bar at the given percentage (0–100).
    pub fn spark_color(&self, pct: f64) -> Color {
        heat_color(pct)
    }

    /// Pick a gauge style based on percentage (0–100).
    pub fn gauge_for_pct(&self, pct: f64) -> Style {
        Style::default().fg(heat_color(pct))
    }

    // ── Compiled-in fallback ──────────────────────────────────────────────────

    pub fn default_dark() -> Self {
        Self {
            border:         Style::default().fg(Color::DarkGray),
            border_focused: Style::default().fg(Color::Cyan),
            title:          Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            header:         Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),

            gauge_high: Style::default().fg(Color::Red),

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
            spark_chars: SPARK_CHARS,
        }
    }

    /// Monochrome fallback when NO_COLOR is set - no colour at all.
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

            gauge_high: bold,

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
            spark_chars: ASCII_SPARK_CHARS,
        }
    }
}
