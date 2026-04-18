use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::glyphs::Glyphs;
use crate::models::memory::MemSnapshot;
use crate::ui::gauge_bar;
use crate::ui::theme::Theme;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    snapshot: &MemSnapshot,
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
            format!(" {}Memory ", glyphs.mem_icon),
            theme.title,
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let has_spark = inner.height >= 5;

    let constraints: Vec<Constraint> = if has_spark {
        vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ]
    } else {
        vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ]
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (row, (prefix, used, total, pct)) in [
        ("RAM ", snapshot.ram_used_bytes,    snapshot.ram_total_bytes,    snapshot.ram_pct()),
        ("SWP ", snapshot.swap_used_bytes,   snapshot.swap_total_bytes,   snapshot.swap_pct()),
        ("COM ", snapshot.commit_total_bytes, snapshot.commit_limit_bytes, snapshot.commit_pct()),
    ]
    .iter()
    .enumerate()
    {
        let label = format!(
            "{}{} / {} ({:.1}%)",
            prefix,
            ByteSize(*used),
            ByteSize(*total),
            pct
        );
        gauge_bar::render_bar(
            frame,
            rows[row],
            &label,
            pct / 100.0,
            theme.gauge_for_pct(*pct),
            theme.text_normal,
            &theme.gauge_style,
        );
    }

    if has_spark {
        let spark_area = rows[4];
        render_sparkline(frame, spark_area, snapshot, theme);
    }
}

fn render_sparkline(frame: &mut Frame, area: Rect, snapshot: &MemSnapshot, theme: &Theme) {
    let history = &snapshot.ram_history.data;
    let label = format!("RAM {:>5.1}%", snapshot.ram_pct());

    let block = Block::default().title(Span::styled(label, theme.text_dim));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 {
        return;
    }

    let w = inner.width as usize;
    let offset = history.len().saturating_sub(w);

    let spans: Vec<Span> = (0..w)
        .map(|col| {
            let v = history.get(offset + col).copied().unwrap_or(0.0) as f64;
            let idx = ((v / 100.0) * 8.0).round().clamp(0.0, 8.0) as usize;
            let color = theme.spark_color(v);
            Span::styled(theme.spark_chars[idx], Style::default().fg(color))
        })
        .collect();

    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}
