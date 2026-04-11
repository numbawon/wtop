use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Span,
    widgets::{Block, Borders, Gauge},
    Frame,
};

use crate::models::memory::MemSnapshot;
use crate::ui::theme::Theme;

pub fn render(frame: &mut Frame, area: Rect, snapshot: &MemSnapshot, theme: &Theme, focused: bool) {
    let border_style = if focused { theme.border_focused } else { theme.border };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(" Memory ", theme.title));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Three rows: RAM, Swap, Commit.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    // RAM
    render_bar(
        frame,
        rows[0],
        "RAM ",
        snapshot.ram_used_bytes,
        snapshot.ram_total_bytes,
        snapshot.ram_pct(),
        theme,
    );

    // Swap
    render_bar(
        frame,
        rows[1],
        "SWP ",
        snapshot.swap_used_bytes,
        snapshot.swap_total_bytes,
        snapshot.swap_pct(),
        theme,
    );

    // Commit charge
    render_bar(
        frame,
        rows[2],
        "COM ",
        snapshot.commit_total_bytes,
        snapshot.commit_limit_bytes,
        snapshot.commit_pct(),
        theme,
    );
}

fn render_bar(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    used: u64,
    total: u64,
    pct: f64,
    theme: &Theme,
) {
    let used_str = ByteSize(used).to_string();
    let total_str = ByteSize(total).to_string();
    let display = format!("{label}{used_str} / {total_str} ({pct:.1}%)");

    let gauge = Gauge::default()
        .gauge_style(theme.gauge_for_pct(pct))
        .label(display)
        .ratio((pct / 100.0).clamp(0.0, 1.0));

    frame.render_widget(gauge, area);
}
