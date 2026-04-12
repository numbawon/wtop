use ratatui::{
    layout::{Alignment, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::ui::theme::Theme;

pub fn render(frame: &mut Frame, area: Rect, pid: u32, name: &str, theme: &Theme) {
    let width = 50u16.min(area.width);
    let height = 7u16.min(area.height);

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    let proc_line = format!("\"{}\"  (PID: {})", name, pid);
    let body = vec![
        Line::from(""),
        Line::from(Span::styled(proc_line, theme.text_bright)),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Enter]", theme.text_bright),
            Span::styled(" Confirm  ", theme.text_dim),
            Span::styled("[Esc]", theme.text_bright),
            Span::styled(" Cancel", theme.text_dim),
        ]),
    ];

    let para = Paragraph::new(body)
        .alignment(Alignment::Center)
        .style(theme.panel_bg)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(theme.border_set.clone())
                .border_style(theme.gauge_high)
                .title(Span::styled(" Kill Process? ", theme.title)),
        );

    frame.render_widget(Clear, popup);
    frame.render_widget(para, popup);
}
