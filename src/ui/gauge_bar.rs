//! Unified bar/gauge renderer — wraps ratatui Gauge, LineGauge, and custom
//! Segmented / ASCII styles behind a single `render_bar()` call.

use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    symbols,
    text::{Line, Span},
    widgets::{Gauge, LineGauge, Paragraph},
    Frame,
};

use crate::config::GaugeStyle;

/// Render a single horizontal bar inside `area` (typically 1 row high).
///
/// - `label`      — descriptive text shown inside/beside the bar
/// - `ratio`      — fill fraction in 0.0–1.0
/// - `fill_style` — colour for the filled portion (from `theme.gauge_for_pct`)
/// - `text_style` — colour for label text overlaid on the bar (use `theme.text_normal`)
/// - `gauge_style`— which visual variant to render
pub fn render_bar(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    ratio: f64,
    fill_style: Style,
    text_style: Style,
    gauge_style: &GaugeStyle,
) {
    let ratio = ratio.clamp(0.0, 1.0);

    match gauge_style {
        GaugeStyle::Block => {
            let g = Gauge::default()
                .gauge_style(fill_style)
                .label(label)
                .ratio(ratio);
            frame.render_widget(g, area);
        }

        GaugeStyle::Line => {
            let g = LineGauge::default()
                .filled_style(fill_style)
                .label(Line::from(Span::styled(label, text_style)))
                .ratio(ratio)
                .line_set(symbols::line::THICK);
            frame.render_widget(g, area);
        }

        GaugeStyle::Segmented => {
            // Phase 1 — draw the bar (filled blocks + empty space).
            let w = area.width as usize;
            let bar = build_block_bar(ratio, w);
            let bar_para = Paragraph::new(Span::styled(bar, fill_style));
            frame.render_widget(bar_para, area);

            // Phase 2 — overlay the label centred over the bar.
            // ratatui renders widgets into a shared buffer; the label chars
            // simply overwrite the bar chars at those cell positions, producing
            // the classic "text floating on gauge" look.
            let label_para = Paragraph::new(Span::styled(label, text_style))
                .alignment(Alignment::Center);
            frame.render_widget(label_para, area);
        }

        GaugeStyle::Ascii => {
            let w = area.width as usize;
            // Format: "[========   ] label"
            // Reserve 2 chars for [ ], a space after ], then the label.
            let label_part = format!(" {label}");
            let bracket_inner = w
                .saturating_sub(2 + label_part.len())
                .max(4);
            let filled = ((ratio * bracket_inner as f64) as usize).min(bracket_inner);
            let empty = bracket_inner - filled;
            let bar = format!(
                "[{}{}]{}",
                "=".repeat(filled),
                " ".repeat(empty),
                label_part
            );
            let para = Paragraph::new(Span::styled(bar, fill_style));
            frame.render_widget(para, area);
        }
    }
}

/// Build a string of sub-cell Unicode block elements that fills `width` columns
/// to the given `ratio`. Uses fractional blocks (▏▎▍▌▋▊▉█) for smooth edges.
pub fn build_block_bar(ratio: f64, width: usize) -> String {
    // 8 fractional steps per cell
    const EIGHTHS: &[char] = &[' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

    let total_eighths = ((ratio * width as f64) * 8.0).round() as usize;
    let full = (total_eighths / 8).min(width);
    let frac = total_eighths % 8;

    let mut s = String::with_capacity(width);
    for _ in 0..full {
        s.push('█');
    }
    if full < width {
        s.push(EIGHTHS[frac]);
        for _ in (full + 1)..width {
            s.push(' ');
        }
    }
    s
}
