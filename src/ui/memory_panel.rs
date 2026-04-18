use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    text::Span,
    widgets::{Block, Borders},
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

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
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
}
