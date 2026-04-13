//! Jump-to-PID input overlay — shown when the user presses Ctrl+G.
//!
//! A small centered box with a digit input field. Enter confirms, Esc cancels.

use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::ui::theme::Theme;

const PANEL_W: u16 = 32;
const PANEL_H: u16 = 5;

pub fn render(frame: &mut Frame, area: Rect, input: &str, not_found: bool, theme: &Theme) {
    let x = area.x + area.width.saturating_sub(PANEL_W) / 2;
    let y = area.y + area.height.saturating_sub(PANEL_H) / 2;
    let rect = Rect {
        x,
        y,
        width: PANEL_W.min(area.width),
        height: PANEL_H.min(area.height),
    };

    let cursor = if (frame.count() / 10) % 2 == 0 { "▌" } else { " " };
    let input_line = Line::from(vec![
        Span::styled("  PID: ", theme.header),
        Span::styled(input, theme.text_bright),
        Span::styled(cursor, theme.text_bright),
    ]);

    let hint_line = if not_found {
        Line::from(vec![
            Span::raw("  "),
            Span::styled("PID not found", theme.gauge_high),
        ])
    } else {
        Line::from(vec![
            Span::raw("  "),
            Span::styled("[Enter]", theme.text_bright),
            Span::styled(" jump  ", theme.text_dim),
            Span::styled("[Esc]", theme.text_bright),
            Span::styled(" cancel", theme.text_dim),
        ])
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(theme.border_set.clone())
        .border_style(theme.border_focused)
        .title(Span::styled(" Jump to PID ", theme.title))
        .style(theme.panel_bg);

    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(vec![Line::from(""), input_line, Line::from(""), hint_line])
            .block(block),
        rect,
    );
}
