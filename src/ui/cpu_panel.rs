use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::glyphs::Glyphs;
use crate::models::cpu::CpuSnapshot;
use crate::ui::gauge_bar;
use crate::ui::theme::Theme;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    snapshot: &CpuSnapshot,
    theme: &Theme,
    glyphs: &Glyphs,
    focused: bool,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set)
        .border_style(border_style)
        .title(Span::styled(
            format!(" {}CPU - {} ", glyphs.cpu_icon, snapshot.brand),
            theme.title,
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if snapshot.cores.is_empty() {
        return;
    }

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

    render_core_bars(frame, bars_area, snapshot, theme, glyphs);
    render_sparkline(frame, spark_area, snapshot, theme);
}

fn render_core_bars(
    frame: &mut Frame,
    area: Rect,
    snapshot: &CpuSnapshot,
    theme: &Theme,
    glyphs: &Glyphs,
) {
    let _ = glyphs;
    let core_count = snapshot.cores.len();
    if core_count == 0 || area.height == 0 {
        return;
    }

    let min_bar_width = 14u16;
    let max_cols_by_width = (area.width / min_bar_width).max(1) as usize;
    let max_rows = area.height as usize;
    let cols = best_col_count(core_count, max_cols_by_width, max_rows);
    let rows = core_count.div_ceil(cols);

    let row_constraints: Vec<Constraint> = (0..rows)
        .map(|_| Constraint::Length(1))
        .collect();

    let row_rects = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(area);

    // All rows use the same `cols`-wide grid so bars stay visually aligned.
    let col_constraints: Vec<Constraint> =
        (0..cols).map(|_| Constraint::Ratio(1, cols as u32)).collect();

    for (row_i, row_rect) in row_rects.iter().enumerate() {
        let col_rects = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints.clone())
            .split(*row_rect);

        for col_i in 0..cols {
            let core_idx = row_i * cols + col_i;
            if core_idx >= core_count {
                break;
            }
            let core = &snapshot.cores[core_idx];
            let pct = core.usage_pct as f64;
            let label = format!("[{:>2}] {:>5.1}%", core.index, core.usage_pct);
            gauge_bar::render_bar(
                frame,
                col_rects[col_i],
                &label,
                pct / 100.0,
                theme.gauge_for_pct(pct),
                theme.text_normal,
                &theme.gauge_style,
            );
        }
    }
}

/// Pick the largest column count ≤ `max_by_width` that:
/// 1. Produces rows that fit in `max_rows`
/// 2. Divides `core_count` evenly (preferred) or leaves the fewest orphan slots
fn best_col_count(core_count: usize, max_by_width: usize, max_rows: usize) -> usize {
    let cap = max_by_width.min(core_count);
    if cap == 0 {
        return 1;
    }

    let mut best_cols = 1usize;
    let mut best_orphans = usize::MAX;

    // Iterate largest → smallest; stop as soon as we find a perfect divisor.
    for c in (1..=cap).rev() {
        let rows_needed = core_count.div_ceil(c);
        if rows_needed > max_rows {
            continue;
        }
        let orphans = if core_count.is_multiple_of(c) { 0 } else { c - (core_count % c) };
        if orphans < best_orphans {
            best_cols = c;
            best_orphans = orphans;
        }
        if best_orphans == 0 {
            break; // perfect even grid found - no need to check smaller
        }
    }

    best_cols
}

fn render_sparkline(
    frame: &mut Frame,
    area: Rect,
    snapshot: &CpuSnapshot,
    theme: &Theme,
) {
    let history = &snapshot.aggregate_history.data;
    let label = format!("Aggregate {:>5.1}%", snapshot.aggregate_pct);

    let block = Block::default()
        .title(Span::styled(label, theme.text_dim));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let w = inner.width as usize;
    let offset = history.len().saturating_sub(w);

    let spans: Vec<Span> = (0..w)
        .map(|col| {
            let v = history.get(offset + col).copied().unwrap_or(0.0) as u64;
            let idx = ((v as f64 / 100.0) * 8.0).round().clamp(0.0, 8.0) as usize;
            let color = theme.spark_color(v as f64);
            Span::styled(theme.spark_chars[idx], Style::default().fg(color))
        })
        .collect();

    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}
