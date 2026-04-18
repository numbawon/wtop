use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::ui::theme::Theme;

pub fn render(frame: &mut Frame, area: Rect, filter_text: &str, active: bool, theme: &Theme) {
    let style = if active { theme.filter_active } else { theme.filter_inactive };

    let label = if active {
        if filter_text.is_empty() { "Filter: " } else { "Filter: (esc clears) " }
    } else {
        "Filter: (f to activate)"
    };

    let cursor = if active { "▌" } else { "" };

    let line = Line::from(vec![
        Span::styled(label, style),
        Span::styled(filter_text, style),
        Span::styled(cursor, style),
    ]);

    let para = Paragraph::new(line).style(theme.panel_bg);
    frame.render_widget(para, area);
}
