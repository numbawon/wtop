use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Span,
    widgets::{Block, Borders, Gauge, Sparkline},
    Frame,
};

use crate::models::cpu::CpuSnapshot;
use crate::ui::theme::Theme;

pub fn render(frame: &mut Frame, area: Rect, snapshot: &CpuSnapshot, theme: &Theme, focused: bool) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            format!(" CPU — {} ", snapshot.brand),
            theme.title,
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if snapshot.cores.is_empty() {
        return;
    }

    // Split inner area: top portion for core bars, bottom row for sparkline.
    let sparkline_height = 2u16;
    if inner.height <= sparkline_height {
        return;
    }

    let splits = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(sparkline_height),
        ])
        .split(inner);

    let bars_area = splits[0];
    let spark_area = splits[1];

    render_core_bars(frame, bars_area, snapshot, theme);
    render_sparkline(frame, spark_area, snapshot, theme);
}

fn render_core_bars(
    frame: &mut Frame,
    area: Rect,
    snapshot: &CpuSnapshot,
    theme: &Theme,
) {
    let core_count = snapshot.cores.len();
    if core_count == 0 || area.height == 0 {
        return;
    }

    // How many cores fit per row given a minimum bar width of ~20 chars?
    let min_bar_width = 20u16;
    let cols = (area.width / min_bar_width).max(1) as usize;
    let rows = (core_count + cols - 1) / cols;

    let row_constraints: Vec<Constraint> = (0..rows)
        .map(|_| Constraint::Length(1))
        .collect();

    let row_rects = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(area);

    for (row_i, row_rect) in row_rects.iter().enumerate() {
        let start = row_i * cols;
        let end = (start + cols).min(core_count);
        let cores_in_row = end - start;

        let col_constraints: Vec<Constraint> =
            (0..cores_in_row).map(|_| Constraint::Ratio(1, cores_in_row as u32)).collect();

        let col_rects = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints)
            .split(*row_rect);

        for (col_i, core) in snapshot.cores[start..end].iter().enumerate() {
            let pct = core.usage_pct as f64;
            let gauge = Gauge::default()
                .gauge_style(theme.gauge_for_pct(pct))
                .label(format!(
                    "[{:>2}] {:>5.1}%",
                    core.index, core.usage_pct
                ))
                .ratio((pct / 100.0).clamp(0.0, 1.0));

            frame.render_widget(gauge, col_rects[col_i]);
        }
    }
}

fn render_sparkline(
    frame: &mut Frame,
    area: Rect,
    snapshot: &CpuSnapshot,
    theme: &Theme,
) {
    let data = snapshot.aggregate_history.as_u64_vec();
    let label = format!("Aggregate {:>5.1}%", snapshot.aggregate_pct);

    let spark = Sparkline::default()
        .data(&data)
        .style(theme.sparkline)
        .block(
            Block::default()
                .title(Span::styled(label, theme.text_dim)),
        )
        .max(100);

    frame.render_widget(spark, area);
}
