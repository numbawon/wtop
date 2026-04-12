use ratatui::{
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Borders, Cell, Clear, Row, Table},
    Frame,
};

use crate::ui::theme::Theme;

const HELP: &[(&str, &str)] = &[
    ("↑/↓  j/k",      "Navigate process list"),
    ("PgUp/PgDn",      "Page up/down"),
    ("Enter",          "Expand/collapse thread list"),
    ("Tab / Shift+Tab","Cycle panel focus"),
    ("s / S",          "Cycle sort column fwd/back"),
    ("r",              "Toggle sort ascending/descending"),
    ("f",              "Open filter bar (Esc to close)"),
    ("k",              "Kill selected process"),
    ("p",              "Toggle system processes"),
    ("u",              "Filter by current user"),
    ("+  /  -",        "Increase/decrease refresh rate"),
    ("? / h",          "Toggle this help"),
    ("g",              "Toggle Nerd Font glyphs on/off"),
    ("T",              "Cycle color theme"),
    ("L",              "Cycle layout mode (Auto/Compact/Wide/Stacked)"),
    ("d",              "Toggle Disk panel visibility"),
    ("n",              "Toggle Network panel visibility"),
    ("c",              "Toggle Disk I/O columns in process table"),
    ("w",              "Windows Terminal info / Nerd Font"),
    ("C",              "Config / settings panel"),
    ("q  Ctrl+C",      "Quit"),
];

pub fn render(frame: &mut Frame, area: Rect, theme: &Theme) {
    let width = 58u16.min(area.width);
    let height = (HELP.len() as u16 + 4).min(area.height);

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    let rows: Vec<Row> = HELP
        .iter()
        .map(|(key, desc)| {
            Row::new(vec![
                Cell::from(*key).style(theme.text_bright),
                Cell::from(*desc).style(theme.text_normal),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [Constraint::Length(18), Constraint::Min(1)],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(theme.border_set.clone())
            .border_style(theme.border_focused)
            .title(Span::styled(" Keyboard Shortcuts ", theme.title)),
    );

    frame.render_widget(Clear, popup);
    frame.render_widget(table, popup);
}
