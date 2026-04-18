use bytesize::ByteSize;
use ratatui::{
    layout::Constraint,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::glyphs::Glyphs;
use crate::models::gpu::GpuAdapter;
use crate::ui::theme::Theme;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    gpus: &[GpuAdapter],
    theme: &Theme,
    glyphs: &Glyphs,
    focused: bool,
) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let header = Row::new(vec![
        Cell::from("GPU").style(theme.header),
        Cell::from("UTIL%").style(theme.header),
        Cell::from("VRAM").style(theme.header),
    ]).height(1);

    let rows: Vec<Row> = if gpus.is_empty() {
        vec![Row::new(vec![
            Cell::from("No GPU detected"),
            Cell::from(""),
            Cell::from(""),
        ]).style(theme.text_dim)]
    } else {
        gpus.iter().map(|g| {
            let util = g.utilization_pct as f64;
            let util_cell = Cell::from(build_spark_line(g, util, theme));

            let vram_str = if g.vram_total_bytes > 0 {
                format!(
                    "{} / {}",
                    ByteSize(g.vram_used_bytes),
                    ByteSize(g.vram_total_bytes),
                )
            } else if g.vram_used_bytes > 0 {
                ByteSize(g.vram_used_bytes).to_string()
            } else {
                "-".to_string()
            };

            let vram_pct = if g.vram_total_bytes > 0 {
                g.vram_used_bytes as f64 / g.vram_total_bytes as f64 * 100.0
            } else {
                0.0
            };
            let vram_style = if g.vram_total_bytes > 0 {
                theme.gauge_for_pct(vram_pct)
            } else {
                theme.text_dim
            };

            Row::new(vec![
                Cell::from(g.name.as_str()),
                util_cell,
                Cell::from(vram_str).style(vram_style),
            ]).style(theme.row_normal)
        }).collect()
    };

    let table = Table::new(rows, [
        Constraint::Min(12),
        Constraint::Length(14),
        Constraint::Length(18),
    ])
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(theme.border_set)
            .border_style(border_style)
            .title(Span::styled(
                format!(" {}GPU ", glyphs.gpu_icon),
                theme.title,
            )),
    );

    frame.render_widget(table, area);
}

fn build_spark_line<'a>(g: &GpuAdapter, util: f64, theme: &'a Theme) -> Line<'a> {
    let history = &g.util_history.data;
    let spark_len = 8usize;
    let offset = history.len().saturating_sub(spark_len);

    let mut spans: Vec<Span<'a>> = (0..spark_len)
        .map(|i| {
            let v = history.get(offset + i).copied().unwrap_or(0.0) as f64;
            let idx = ((v / 100.0) * 8.0).round().clamp(0.0, 8.0) as usize;
            Span::styled(theme.spark_chars[idx], Style::default().fg(theme.spark_color(v)))
        })
        .collect();

    spans.push(Span::styled(
        format!(" {:4.1}%", util),
        theme.gauge_for_pct(util),
    ));

    Line::from(spans)
}
