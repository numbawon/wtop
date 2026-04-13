//! Network adapter filter overlay — shows all detected adapters as a toggle list.
//!
//! Opened via the Config panel ("Adapter Filters →").
//! Up/Down navigate, Space or Enter toggles, Esc saves and closes.

use bytesize::ByteSize;
use ratatui::{
    layout::{Constraint, Rect},
    text::Span,
    widgets::{Block, Borders, Cell, Clear, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table},
    Frame,
};

use crate::models::network::NetSnapshot;
use crate::ui::theme::Theme;

const PANEL_W: u16 = 62;
/// Maximum visible adapter rows (not counting header/hint).
const MAX_VISIBLE: usize = 18;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    adapters: &[NetSnapshot],
    hidden: &[String],
    cursor: usize,
    theme: &Theme,
) {
    let total = adapters.len();
    let visible_rows = MAX_VISIBLE.min(total).max(1);

    // Scroll offset: keep cursor in view.
    let scroll = if cursor >= visible_rows {
        cursor - visible_rows + 1
    } else {
        0
    };

    // Panel height: border(2) + header(1) + visible rows + hint(1).
    let panel_h = (visible_rows + 4) as u16;
    let panel_h = panel_h.min(area.height.saturating_sub(2));

    let x = area.x + area.width.saturating_sub(PANEL_W) / 2;
    let y = area.y + area.height.saturating_sub(panel_h) / 2;
    let rect = Rect {
        x,
        y,
        width: PANEL_W.min(area.width),
        height: panel_h,
    };

    let header = Row::new(vec![
        Cell::from("").style(theme.header),
        Cell::from("ADAPTER").style(theme.header),
        Cell::from("TYPE").style(theme.header),
        Cell::from("RX/s").style(theme.header),
        Cell::from("TX/s").style(theme.header),
    ])
    .height(1);

    let mut rows: Vec<Row> = Vec::new();

    if adapters.is_empty() {
        rows.push(
            Row::new(vec![
                Cell::from(""),
                Cell::from("No adapters detected"),
                Cell::from(""),
                Cell::from(""),
                Cell::from(""),
            ])
            .style(theme.text_dim),
        );
    } else {
        let end = (scroll + visible_rows).min(total);
        for (idx, n) in adapters[scroll..end].iter().enumerate() {
            let abs_idx = scroll + idx;
            let is_hidden = hidden.contains(&n.display_name);
            let check = if is_hidden { "[ ]" } else { "[✓]" };
            let kind = if n.is_virtual { "virtual" } else { "physical" };
            let row_style = if abs_idx == cursor {
                theme.row_selected
            } else if is_hidden {
                theme.text_dim
            } else {
                theme.row_normal
            };
            rows.push(
                Row::new(vec![
                    Cell::from(check),
                    Cell::from(super::truncate(&n.display_name, 22)),
                    Cell::from(kind),
                    Cell::from(ByteSize(n.rx_bps).to_string() + "/s"),
                    Cell::from(ByteSize(n.tx_bps).to_string() + "/s"),
                ])
                .style(row_style),
            );
        }
    }

    // Hint row.
    rows.push(
        Row::new(vec![
            Cell::from(""),
            Cell::from(Span::styled(
                "↑↓ nav   Space/Enter toggle   Esc close",
                theme.text_dim,
            )),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
        ]),
    );

    let title = if total > visible_rows {
        format!(" Network Adapter Filters  ({}/{}) ", cursor + 1, total)
    } else {
        " Network Adapter Filters ".to_string()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Min(22),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_set(theme.border_set.clone())
            .border_style(theme.border_focused)
            .title(Span::styled(title, theme.title))
            .style(theme.panel_bg),
    );

    frame.render_widget(Clear, rect);
    frame.render_widget(table, rect);

    // Scrollbar — only when list overflows.
    if total > visible_rows {
        let sb_rect = Rect {
            x: rect.x + rect.width - 1,
            y: rect.y + 2, // skip top border + header
            width: 1,
            height: visible_rows as u16,
        };
        let mut sb_state = ScrollbarState::new(total.saturating_sub(visible_rows))
            .position(scroll);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .thumb_style(theme.border_focused)
                .track_style(theme.text_dim),
            sb_rect,
            &mut sb_state,
        );
    }
}
